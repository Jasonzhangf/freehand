use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_config::{
    AgentMode, ProviderAuthSourceKind, ProviderAuthType, ProviderProtocol, ProviderType,
    SelectedAgentConfig, SelectedPeerAgentConfig, SelectedProviderConfig,
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
        master_live_request(&runtime_home, "worker", "{}", &task, first, 0).expect("first request");
    let last_request =
        master_live_request(&runtime_home, "worker", "{}", &task, last, 0).expect("last request");

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
fn production_master_runner_requires_interrupted_assignment_decision() {
    let runtime_home = temp_path("interrupted-missing-decision");
    bootstrap_runner_with_selected(
        &runtime_home,
        selected_master_with_workers(&["worker-alpha"]),
    );
    let task_id = seed_interrupted_task(&runtime_home, "worker-alpha");
    let executor = Arc::new(StubMasterExecutor::new(|request| {
        assert!(
            request
                .prompt
                .contains("Agent is a reusable execution resource in the pool")
        );
        assert!(
            request
                .prompt
                .contains("do not create a duplicate task for the same objective")
        );
        Ok("prose without assignment is invalid".to_owned())
    }));
    let runner = test_runner_with_selected(
        runtime_home.clone(),
        selected_master_with_workers(&["worker-alpha"]),
        executor.clone(),
    );

    assert!(matches!(
        runner
            .run_once()
            .expect_err("prose-only interrupted decision must fail"),
        ProductionMasterRunnerError::MissingInterruptedDecision { .. }
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        TaskRuntime::boot(&runtime_home, AgentId::new("master"))
            .expect("runtime")
            .query_task(&task_id)
            .expect("task")
            .status,
        TaskStatus::Interrupted
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_can_take_over_interrupted_task_to_another_worker() {
    let runtime_home = temp_path("interrupted-takeover");
    let selected = selected_master_with_workers(&["worker-alpha", "worker-gamma"]);
    bootstrap_runner_with_selected(&runtime_home, selected.clone());
    let parent_session_id = SessionId::new("parent-session-takeover");
    let task_id = seed_interrupted_task_with_parent(
        &runtime_home,
        "worker-gamma",
        Some(parent_session_id.clone()),
    );
    let action_task_id = task_id.clone();
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        assert!(
            request
                .prompt
                .contains("Configured Worker ids: worker-alpha, worker-gamma")
        );
        assert!(request.prompt.contains("\"agent_id\": \"worker-alpha\""));
        assert!(request.prompt.contains("\"agent_id\": \"worker-gamma\""));
        assert!(
            request
                .prompt
                .contains("takeover_to_another_available_configured_worker")
        );
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .assign_task(freehand_task::TaskAssignRequest {
                task_id: action_task_id.clone(),
                agent_id: AgentId::new("worker-alpha"),
                actor: test_actor("master"),
                watermark: test_watermark("takeover-alpha"),
            })
            .map_err(to_string)?;
        Ok("takeover_to_worker=worker-alpha".to_owned())
    }));
    let runner = test_runner_with_selected(runtime_home.clone(), selected, executor.clone());

    assert!(matches!(
        runner.run_once().expect("interrupted takeover tick"),
        ProductionMasterTickOutcome::TaskAdvanced {
            task_id: ref outcome_task_id,
            from: TaskStatus::Interrupted,
            to: TaskStatus::Assigned,
            ..
        } if outcome_task_id == &task_id
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 1);

    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    let task = runtime.query_task(&task_id).expect("task");
    assert_eq!(task.status, TaskStatus::Assigned);
    assert_eq!(
        task.parent.session_id.as_ref().map(SessionId::as_str),
        Some(parent_session_id.as_str())
    );
    assert_eq!(
        task.assignee.as_ref().expect("assignee").agent_id.as_str(),
        "worker-alpha"
    );
    let history = runtime.task_history(&task_id).expect("history");
    let parent_tasks = runtime
        .list_tasks(Default::default())
        .expect("list tasks")
        .into_iter()
        .filter(|candidate| {
            candidate.parent.session_id.as_ref().map(SessionId::as_str)
                == Some(parent_session_id.as_str())
        })
        .collect::<Vec<_>>();
    assert_eq!(parent_tasks.len(), 1);
    assert_eq!(parent_tasks[0].task_id, task_id);
    let assigned_agents: Vec<_> = history
        .iter()
        .filter(|event| event.event_type == "TaskAssigned")
        .filter_map(|event| event.payload["agent_id"].as_str())
        .collect();
    assert_eq!(assigned_agents, vec!["worker-gamma", "worker-alpha"]);
    assert!(
        history
            .iter()
            .any(|event| event.event_type == "TaskInterrupted")
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_handles_worker_attention_as_same_task_adjustment() {
    let runtime_home = temp_path("worker-attention-adjustment");
    let selected = selected_master_with_workers(&["worker-alpha"]);
    bootstrap_runner_with_selected(&runtime_home, selected.clone());
    let task_id = seed_attention_required_task(&runtime_home, "worker-alpha");
    let action_task_id = task_id.clone();
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        assert!(request.prompt.contains("execution_attention_required"));
        assert!(request.prompt.contains("\"severity\": \"critical\""));
        assert!(
            request
                .prompt
                .contains("\"change_kind\": \"task_contract_invalidated\"")
        );
        assert!(
            request
                .prompt
                .contains("do not bury it as a generic blocker")
        );
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .append_task(TaskAppendRequest {
                task_id: action_task_id.clone(),
                note: "attention_resolution: revise acceptance before continuing".to_owned(),
                actor: test_actor("master"),
                watermark: test_watermark("attention-adjustment"),
            })
            .map_err(to_string)?;
        runtime
            .assign_task(freehand_task::TaskAssignRequest {
                task_id: action_task_id.clone(),
                agent_id: AgentId::new("worker-alpha"),
                actor: test_actor("master"),
                watermark: test_watermark("attention-reassign"),
            })
            .map_err(to_string)?;
        Ok("adjusted and reassigned the same task".to_owned())
    }));
    let runner = test_runner_with_selected(runtime_home.clone(), selected, executor);

    assert!(matches!(
        runner.run_once().expect("attention decision"),
        ProductionMasterTickOutcome::TaskAdvanced {
            task_id: ref outcome_task_id,
            from: TaskStatus::Interrupted,
            to: TaskStatus::Assigned,
            ..
        } if outcome_task_id == &task_id
    ));
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    let history = runtime.task_history(&task_id).expect("history");
    assert!(
        history
            .iter()
            .any(|event| event.event_type == "TaskAttentionRequired")
    );
    assert!(history.iter().any(|event| {
        event.event_type == "TaskProgressed"
            && event.payload["note"]
                .as_str()
                .is_some_and(|note| note.starts_with("attention_resolution:"))
    }));
    assert_eq!(
        history
            .iter()
            .filter(|event| event.event_type == "TaskCreated")
            .count(),
        1
    );
    assert!(
        !history
            .iter()
            .any(|event| event.event_type == "TaskBlocked")
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_idle_consumes_highest_priority_attention() {
    let runtime_home = temp_path("idle-priority-attention");
    let selected = selected_master_with_workers(&["worker", "worker-alpha"]);
    bootstrap_runner_with_selected(&runtime_home, selected.clone());
    let review_task_id = seed_review_ready_task(&runtime_home);
    let attention_task_id = seed_attention_required_task(&runtime_home, "worker-alpha");
    let action_attention_task_id = attention_task_id.clone();
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        assert!(request.prompt.contains("execution_attention_required"));
        assert!(request.prompt.contains(action_attention_task_id.as_str()));
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .append_task(TaskAppendRequest {
                task_id: action_attention_task_id.clone(),
                note: "attention_resolution: prioritized before review".to_owned(),
                actor: test_actor("master"),
                watermark: test_watermark("idle-priority-attention"),
            })
            .map_err(to_string)?;
        runtime
            .assign_task(freehand_task::TaskAssignRequest {
                task_id: action_attention_task_id.clone(),
                agent_id: AgentId::new("worker-alpha"),
                actor: test_actor("master"),
                watermark: test_watermark("idle-priority-reassign"),
            })
            .map_err(to_string)?;
        Ok("priority attention handled".to_owned())
    }));
    let runner = test_runner_with_selected(runtime_home.clone(), selected, executor);

    assert!(matches!(
        runner.run_once().expect("priority attention tick"),
        ProductionMasterTickOutcome::TaskAdvanced {
            task_id: ref outcome_task_id,
            from: TaskStatus::Interrupted,
            to: TaskStatus::Assigned,
            ..
        } if outcome_task_id == &attention_task_id
    ));
    let runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    assert_eq!(
        runtime
            .query_task(&review_task_id)
            .expect("review task")
            .status,
        TaskStatus::ReviewSubmitted
    );
    assert_eq!(
        runtime
            .query_task(&attention_task_id)
            .expect("attention task")
            .status,
        TaskStatus::Assigned
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn master_attention_dequeue_gives_blocked_and_task_priority_large_weight() {
    let ordinary_review = test_attention_item("review", "review_ready", None, 100, 0);
    let fresh_critical_attention = test_attention_item(
        "critical",
        "execution_attention_required",
        Some("critical"),
        0,
        1,
    );
    assert_eq!(
        highest_priority_attention_index(&[ordinary_review, fresh_critical_attention], 2),
        Some(1),
        "fresh critical attention must outrank ordinary review work"
    );

    let high_priority_review = test_attention_item("review-high", "review_ready", None, 100, 0);
    let ordinary_blocked = test_attention_item("blocked", "execution_blocked", None, 0, 1);
    assert_eq!(
        highest_priority_attention_index(&[high_priority_review, ordinary_blocked], 2),
        Some(1),
        "a fresh blocked showstopper must outrank an ordinary review even when the review task priority is high"
    );

    let low_priority_blocked =
        test_attention_item("blocked-low", "execution_blocked", None, -100, 0);
    let high_priority_blocked =
        test_attention_item("blocked-high", "execution_blocked", None, 100, 1);
    assert_eq!(
        highest_priority_attention_index(&[low_priority_blocked, high_priority_blocked], 2),
        Some(1),
        "task priority must materially order otherwise-equivalent showstopper attention"
    );
}

#[test]
fn master_attention_dequeue_ages_old_low_priority_item_without_starvation() {
    let old_low_priority_review = test_attention_item("review-old", "review_ready", None, -100, 0);
    let fresh_critical_attention = test_attention_item(
        "critical-fresh",
        "execution_attention_required",
        Some("critical"),
        100,
        110,
    );

    assert_eq!(
        highest_priority_attention_index(&[old_low_priority_review, fresh_critical_attention], 111,),
        Some(0),
        "deterministic admission aging must eventually surface old low-priority work even under fresh critical arrivals"
    );
}

#[test]
fn master_attention_admission_preserves_event_inbox_order() {
    let runtime_home = temp_path("attention-admission-order");
    let selected = selected_master_with_workers(&["worker", "worker-alpha"]);
    bootstrap_runner_with_selected(&runtime_home, selected.clone());
    seed_review_ready_task(&runtime_home);
    seed_attention_required_task(&runtime_home, "worker-alpha");
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Err("hold selected attention for state inspection".to_owned())
    }));
    let runner = test_runner_with_selected(runtime_home.clone(), selected, executor);
    let task_runtime = TaskRuntime::boot(&runtime_home, AgentId::new("master")).expect("runtime");
    let before = runner.load_state().expect("state before admission");
    let inbox = task_runtime
        .query_event_inbox(freehand_task::TaskEventInboxQuery {
            after_cursor: before.cursor,
            limit: usize::MAX,
        })
        .expect("event inbox");
    let expected_event_ids = inbox
        .events
        .iter()
        .filter(|event| master_event_requires_attention(&event.kind))
        .map(|event| event.event_id.clone())
        .collect::<Vec<_>>();

    assert!(matches!(
        runner
            .run_once()
            .expect_err("selected attention is intentionally held"),
        ProductionMasterRunnerError::Execution(_)
    ));
    let admitted = runner.load_state().expect("admitted state");
    assert_eq!(
        admitted
            .pending_attention
            .iter()
            .map(|item| item.event.event_id.clone())
            .collect::<Vec<_>>(),
        expected_event_ids,
        "priority selection must not reorder durable EventInbox admission"
    );
    assert_eq!(
        admitted
            .pending_attention
            .iter()
            .map(|item| item.admitted_sequence)
            .collect::<Vec<_>>(),
        (0..admitted.pending_attention.len() as u64).collect::<Vec<_>>()
    );
    assert_eq!(admitted.cursor, inbox.next_cursor);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn master_attention_retry_keeps_same_pending_item() {
    let runtime_home = temp_path("attention-retry-same-item");
    bootstrap_runner(&runtime_home);
    seed_review_ready_task(&runtime_home);
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Err("retryable provider failure".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor);

    assert!(matches!(
        runner.run_once().expect_err("first retryable failure"),
        ProductionMasterRunnerError::Execution(_)
    ));
    let after_first = runner.load_state().expect("state after first failure");
    assert_eq!(after_first.pending_attention.len(), 1);
    let first_event_id = after_first.pending_attention[0].event.event_id.clone();
    let first_admitted_sequence = after_first.pending_attention[0].admitted_sequence;
    let first_cursor = after_first.cursor.clone();
    let first_next_sequence = after_first.next_attention_sequence;

    assert!(matches!(
        runner.run_once().expect_err("second retryable failure"),
        ProductionMasterRunnerError::Execution(_)
    ));
    let after_second = runner.load_state().expect("state after second failure");
    assert_eq!(after_second.pending_attention.len(), 1);
    assert_eq!(
        after_second.pending_attention[0].event.event_id,
        first_event_id
    );
    assert_eq!(
        after_second.pending_attention[0].admitted_sequence,
        first_admitted_sequence
    );
    assert_eq!(after_second.cursor, first_cursor);
    assert_eq!(
        after_second.next_attention_sequence, first_next_sequence,
        "retry must not re-admit or age the same event by duplication"
    );
    assert_eq!(after_second.retry_attempt, 2);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_busy_defers_lower_priority_attention() {
    let runtime_home = temp_path("busy-defers-low-priority");
    bootstrap_runner(&runtime_home);
    seed_review_ready_task(&runtime_home);
    register_test_active_master_work(&runtime_home, "busy-parent", "runtime-turn-42");
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Err("review attention must not interrupt foreground work".to_owned())
    }));
    let runner = test_runner(runtime_home.clone(), executor.clone());

    assert_eq!(
        runner.run_once().expect("busy tick"),
        ProductionMasterTickOutcome::Idle
    );
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);
    let state = runner.load_state().expect("state");
    assert_eq!(
        state.pending_attention.len(),
        1,
        "deferred attention must remain durable for later dequeue"
    );
    let active = load_master_active_work(&runtime_home, &AgentId::new("master"))
        .expect("load active")
        .expect("active work");
    assert_eq!(active.state, MasterActiveWorkState::Running);
    assert!(active.suspend_requested_by.is_none());
    assert!(active.attention_resolution.is_none());

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_busy_low_priority_never_cancels_active_work() {
    let runtime_home = temp_path("busy-low-priority-no-cancel");
    bootstrap_runner(&runtime_home);
    seed_review_ready_task(&runtime_home);
    register_test_active_master_work(&runtime_home, "busy-parent", "runtime-turn-43");
    let original = load_master_active_work(&runtime_home, &AgentId::new("master"))
        .expect("load active")
        .expect("active work");
    let runner = test_runner(
        runtime_home.clone(),
        Arc::new(StubMasterExecutor::new(|_| {
            Err("executor must not run".to_owned())
        })),
    );

    runner.run_once().expect("busy tick");
    let active = load_master_active_work(&runtime_home, &AgentId::new("master"))
        .expect("load active")
        .expect("active work");
    assert_eq!(active.work_id, original.work_id);
    assert_eq!(active.session_id, original.session_id);
    assert_eq!(active.logical_turn_id, original.logical_turn_id);
    assert_eq!(active.trace_id, original.trace_id);
    assert_eq!(active.state, MasterActiveWorkState::Running);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_busy_never_interrupts_mid_provider_or_mid_tool_effect() {
    let runtime_home = temp_path("busy-mid-effect-no-interrupt");
    let selected = selected_master_with_workers(&["worker-alpha"]);
    bootstrap_runner_with_selected(&runtime_home, selected.clone());
    seed_attention_required_task(&runtime_home, "worker-alpha");
    register_test_active_master_work(&runtime_home, "busy-parent", "runtime-turn-44");
    update_master_active_work_safe_point(
        &runtime_home,
        &AgentId::new("master"),
        MasterWorkSafePoint::ProviderInFlight,
    )
    .expect("mark provider in-flight");
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Err("executor must not run before a safe point".to_owned())
    }));
    let runner = test_runner_with_selected(runtime_home.clone(), selected, executor.clone());

    assert_eq!(
        runner.run_once().expect("provider in-flight tick"),
        ProductionMasterTickOutcome::Idle
    );
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);
    let active = load_master_active_work(&runtime_home, &AgentId::new("master"))
        .expect("load active")
        .expect("active work");
    assert_eq!(active.state, MasterActiveWorkState::SuspendRequested);
    assert_eq!(active.safe_point, MasterWorkSafePoint::ProviderInFlight);
    assert!(active.suspend_requested_by.is_some());
    assert_eq!(
        runner.load_state().expect("state").pending_attention.len(),
        1
    );

    update_master_active_work_safe_point(
        &runtime_home,
        &AgentId::new("master"),
        MasterWorkSafePoint::ToolEffectInFlight,
    )
    .expect("mark tool-effect in-flight");
    assert_eq!(
        runner.run_once().expect("tool effect tick"),
        ProductionMasterTickOutcome::Idle
    );
    let active = load_master_active_work(&runtime_home, &AgentId::new("master"))
        .expect("load active")
        .expect("active work");
    assert_eq!(active.state, MasterActiveWorkState::SuspendRequested);
    assert_eq!(active.safe_point, MasterWorkSafePoint::ToolEffectInFlight);
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_foreground_acknowledges_suspend_at_safe_point() {
    let runtime_home = temp_path("foreground-ack-suspend-safe-point");
    register_test_active_master_work(&runtime_home, "foreground-session", "runtime-turn-90");
    let master_agent_id = AgentId::new("master");
    let mut active = load_master_active_work(&runtime_home, &master_agent_id)
        .expect("load active")
        .expect("active work");
    active.state = MasterActiveWorkState::SuspendRequested;
    active.safe_point = MasterWorkSafePoint::ProviderInFlight;
    active.suspend_requested_by = Some(MasterAttentionReference {
        event_id: "attention-safe-point".to_owned(),
        task_id: TaskId::new("task-safe-point"),
        kind: "task_blocked".to_owned(),
        severity_rank: 5,
        task_priority: 95,
    });
    write_master_active_work_unlocked(&runtime_home, &active).expect("write suspend request");

    let checkpoint = record_master_active_work_safe_point_if_current(
        &runtime_home,
        &master_agent_id,
        &SessionId::new("foreground-session"),
        &TurnId::new("runtime-turn-90"),
        MasterWorkSafePoint::BeforeToolExecution,
    )
    .expect("record safe point")
    .expect("current foreground work");
    assert_eq!(
        checkpoint.state,
        MasterActiveWorkState::SuspendedByAttention
    );
    assert_eq!(
        checkpoint.safe_point,
        MasterWorkSafePoint::BeforeToolExecution
    );

    let isolated = record_master_active_work_safe_point_if_current(
        &runtime_home,
        &master_agent_id,
        &SessionId::new("master-lifecycle-task-safe-point"),
        &TurnId::new("master-lifecycle-task-safe-point-decision"),
        MasterWorkSafePoint::BeforeProviderRequest,
    )
    .expect("isolated control turn must be ignored");
    assert!(isolated.is_none());
    let after_isolated = load_master_active_work(&runtime_home, &master_agent_id)
        .expect("load after isolated")
        .expect("active work");
    assert_eq!(after_isolated.session_id, checkpoint.session_id);
    assert_eq!(after_isolated.logical_turn_id, checkpoint.logical_turn_id);
    assert_eq!(
        after_isolated.state,
        MasterActiveWorkState::SuspendedByAttention
    );
    assert_eq!(
        after_isolated.safe_point,
        MasterWorkSafePoint::BeforeToolExecution
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_foreground_never_suspends_mid_effect() {
    let in_flight_points = [
        MasterWorkSafePoint::ProviderInFlight,
        MasterWorkSafePoint::ToolEffectInFlight,
        MasterWorkSafePoint::TerminalPersistenceInFlight,
    ];

    for (index, safe_point) in in_flight_points.into_iter().enumerate() {
        let runtime_home = temp_path(&format!("foreground-no-mid-effect-suspend-{index}"));
        let session_id = format!("foreground-mid-effect-{index}");
        let turn_id = format!("runtime-turn-9{index}");
        register_test_active_master_work(&runtime_home, &session_id, &turn_id);
        let master_agent_id = AgentId::new("master");
        let mut active = load_master_active_work(&runtime_home, &master_agent_id)
            .expect("load active")
            .expect("active work");
        active.state = MasterActiveWorkState::SuspendRequested;
        active.suspend_requested_by = Some(MasterAttentionReference {
            event_id: format!("attention-mid-effect-{index}"),
            task_id: TaskId::new(format!("task-mid-effect-{index}")),
            kind: "task_blocked".to_owned(),
            severity_rank: 5,
            task_priority: 95,
        });
        write_master_active_work_unlocked(&runtime_home, &active).expect("write suspend request");

        let checkpoint = record_master_active_work_safe_point_if_current(
            &runtime_home,
            &master_agent_id,
            &SessionId::new(session_id),
            &TurnId::new(turn_id),
            safe_point,
        )
        .expect("record in-flight safe point")
        .expect("current foreground work");
        assert_eq!(checkpoint.state, MasterActiveWorkState::SuspendRequested);
        assert_eq!(checkpoint.safe_point, safe_point);
        assert!(checkpoint.attention_resolution.is_none());

        fs::remove_dir_all(runtime_home).expect("cleanup");
    }
}

#[test]
fn production_master_busy_high_priority_interrupts_at_safe_point() {
    let runtime_home = temp_path("busy-high-priority-safe-point");
    let selected = selected_master_with_workers(&["worker-alpha"]);
    bootstrap_runner_with_selected(&runtime_home, selected.clone());
    let task_id = seed_attention_required_task(&runtime_home, "worker-alpha");
    register_test_active_master_work(&runtime_home, "busy-parent", "runtime-turn-45");
    update_master_active_work_safe_point(
        &runtime_home,
        &AgentId::new("master"),
        MasterWorkSafePoint::BeforeToolExecution,
    )
    .expect("mark safe point");
    let action_task_id = task_id.clone();
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        let active = load_master_active_work(&request.runtime_home, &AgentId::new("master"))
            .map_err(to_string)?
            .expect("active work");
        assert_eq!(active.state, MasterActiveWorkState::SuspendedByAttention);
        assert_eq!(active.logical_turn_id.as_str(), "runtime-turn-45");
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .append_task(TaskAppendRequest {
                task_id: action_task_id.clone(),
                note: "attention_resolution: safe-point interruption handled".to_owned(),
                actor: test_actor("master"),
                watermark: test_watermark("busy-attention-append"),
            })
            .map_err(to_string)?;
        runtime
            .assign_task(freehand_task::TaskAssignRequest {
                task_id: action_task_id.clone(),
                agent_id: AgentId::new("worker-alpha"),
                actor: test_actor("master"),
                watermark: test_watermark("busy-attention-reassign"),
            })
            .map_err(to_string)?;
        Ok("safe-point attention handled".to_owned())
    }));
    let runner = test_runner_with_selected(runtime_home.clone(), selected, executor.clone());

    assert!(matches!(
        runner.run_once().expect("safe-point interrupt tick"),
        ProductionMasterTickOutcome::TaskAdvanced {
            task_id: ref outcome_task_id,
            from: TaskStatus::Interrupted,
            to: TaskStatus::Assigned,
            ..
        } if outcome_task_id == &task_id
    ));
    assert_eq!(executor.calls.load(Ordering::Relaxed), 1);
    let active = load_master_active_work(&runtime_home, &AgentId::new("master"))
        .expect("load active")
        .expect("active work");
    assert_eq!(active.state, MasterActiveWorkState::Running);
    assert_eq!(active.safe_point, MasterWorkSafePoint::BetweenRounds);
    assert!(active.suspend_requested_by.is_none());
    assert_eq!(
        active
            .attention_resolution
            .as_ref()
            .expect("resolution")
            .decision_kind,
        "task_advanced"
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_attention_uses_isolated_control_turn() {
    let runtime_home = temp_path("attention-isolated-control-turn");
    let selected = selected_master_with_workers(&["worker-alpha"]);
    bootstrap_runner_with_selected(&runtime_home, selected.clone());
    let task_id = seed_attention_required_task(&runtime_home, "worker-alpha");
    register_test_active_master_work(&runtime_home, "foreground-user-session", "runtime-turn-51");
    update_master_active_work_safe_point(
        &runtime_home,
        &AgentId::new("master"),
        MasterWorkSafePoint::BeforeToolExecution,
    )
    .expect("mark foreground safe point");
    let original = load_master_active_work(&runtime_home, &AgentId::new("master"))
        .expect("load active")
        .expect("foreground active work");
    let observed_request = Arc::new(Mutex::new(None::<LiveReasonTurnRequest>));
    let request_out = Arc::clone(&observed_request);
    let action_task_id = task_id.clone();
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        *request_out.lock().expect("request lock") = Some(request.clone());
        let active = load_master_active_work(&request.runtime_home, &AgentId::new("master"))
            .map_err(to_string)?
            .expect("suspended foreground work");
        assert_eq!(active.state, MasterActiveWorkState::SuspendedByAttention);
        assert_eq!(active.work_id, original.work_id);
        assert_eq!(active.session_id, original.session_id);
        assert_eq!(active.logical_turn_id, original.logical_turn_id);
        assert_eq!(active.trace_id, original.trace_id);
        let attention = active
            .suspend_requested_by
            .as_ref()
            .expect("suspended attention reference");
        assert_eq!(attention.task_id, action_task_id);
        assert_eq!(attention.kind, "execution_attention_required");
        assert_ne!(request.session_id, active.session_id);
        assert_ne!(request.turn_id, active.logical_turn_id);
        assert_ne!(request.trace_id, active.trace_id);
        assert!(request.session_id.as_str().starts_with("master-lifecycle-"));
        assert!(
            request
                .turn_id
                .as_str()
                .contains(&sanitize_identifier(&attention.event_id))
        );
        assert!(request.turn_id.as_str().contains("-attempt-0-decision"));
        assert!(
            request
                .trace_id
                .as_str()
                .contains(&sanitize_identifier(&attention.event_id))
        );

        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .append_task(TaskAppendRequest {
                task_id: action_task_id.clone(),
                note: "attention_resolution: isolated control turn handled".to_owned(),
                actor: test_actor("master"),
                watermark: test_watermark("isolated-control-append"),
            })
            .map_err(to_string)?;
        runtime
            .assign_task(freehand_task::TaskAssignRequest {
                task_id: action_task_id.clone(),
                agent_id: AgentId::new("worker-alpha"),
                actor: test_actor("master"),
                watermark: test_watermark("isolated-control-reassign"),
            })
            .map_err(to_string)?;
        Ok("isolated control turn handled".to_owned())
    }));
    let runner = test_runner_with_selected(runtime_home.clone(), selected, executor);

    assert!(matches!(
        runner.run_once().expect("isolated control turn tick"),
        ProductionMasterTickOutcome::TaskAdvanced {
            task_id: ref outcome_task_id,
            from: TaskStatus::Interrupted,
            to: TaskStatus::Assigned,
            ..
        } if outcome_task_id == &task_id
    ));
    let request = observed_request
        .lock()
        .expect("request lock")
        .clone()
        .expect("captured isolated request");
    assert!(request.session_id.as_str().starts_with("master-lifecycle-"));
    assert_ne!(request.session_id.as_str(), "foreground-user-session");
    assert_ne!(request.turn_id.as_str(), "runtime-turn-51");
    let active = load_master_active_work(&runtime_home, &AgentId::new("master"))
        .expect("load restored active")
        .expect("restored foreground work");
    assert_eq!(active.state, MasterActiveWorkState::Running);
    let resolution = active.attention_resolution.expect("typed resolution");
    assert_eq!(
        resolution.resume_from.session_id.as_str(),
        "foreground-user-session"
    );
    assert_eq!(
        resolution.resume_from.logical_turn_id.as_str(),
        "runtime-turn-51"
    );
    assert_eq!(resolution.changed_task_ids, vec![task_id]);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_attention_raw_transcript_never_enters_user_session() {
    let runtime_home = temp_path("attention-raw-transcript-excluded");
    let selected = selected_master_with_workers(&["worker-alpha"]);
    bootstrap_runner_with_selected(&runtime_home, selected.clone());
    let task_id = seed_attention_required_task(&runtime_home, "worker-alpha");
    let foreground_session = SessionId::new("foreground-raw-session");
    register_test_active_master_work(
        &runtime_home,
        foreground_session.as_str(),
        "runtime-turn-52",
    );
    update_master_active_work_safe_point(
        &runtime_home,
        &AgentId::new("master"),
        MasterWorkSafePoint::BeforeTerminalPersistence,
    )
    .expect("mark foreground safe point");
    let raw_sentinel =
        "raw_control_turn_transcript: worker private text provider_response_payload={secret}";
    let action_task_id = task_id.clone();
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        assert_ne!(request.session_id, foreground_session);
        assert!(request.session_id.as_str().starts_with("master-lifecycle-"));
        let runtime =
            TaskRuntime::boot(&request.runtime_home, AgentId::new("master")).map_err(to_string)?;
        runtime
            .append_task(TaskAppendRequest {
                task_id: action_task_id.clone(),
                note: "attention_resolution: raw control output stayed internal".to_owned(),
                actor: test_actor("master"),
                watermark: test_watermark("raw-control-excluded-append"),
            })
            .map_err(to_string)?;
        runtime
            .assign_task(freehand_task::TaskAssignRequest {
                task_id: action_task_id.clone(),
                agent_id: AgentId::new("worker-alpha"),
                actor: test_actor("master"),
                watermark: test_watermark("raw-control-excluded-reassign"),
            })
            .map_err(to_string)?;
        Ok(raw_sentinel.to_owned())
    }));
    let runner = test_runner_with_selected(runtime_home.clone(), selected, executor);

    assert!(matches!(
        runner.run_once().expect("raw transcript exclusion tick"),
        ProductionMasterTickOutcome::TaskAdvanced {
            task_id: ref outcome_task_id,
            ..
        } if outcome_task_id == &task_id
    ));
    let active = load_master_active_work(&runtime_home, &AgentId::new("master"))
        .expect("load restored active")
        .expect("restored active work");
    let rendered_active = serde_json::to_string(&active).expect("render active checkpoint");
    assert!(
        !rendered_active.contains(raw_sentinel),
        "raw control/provider transcript must not be persisted in master_work"
    );
    let resolution = active.attention_resolution.expect("typed resolution");
    assert!(
        resolution.changed_constraints.is_empty(),
        "executor prose must not be copied into typed resolution constraints"
    );
    let persistence = ReasonPersistence::new(runtime_home.clone(), AgentId::new("master"));
    assert!(
        matches!(
            persistence.restore_turn_snapshots_for_ui(&SessionId::new("foreground-raw-session")),
            Err(ReasonPersistenceError::MissingRecoveryTruth(_))
        ),
        "isolated control summary must not create or mutate the foreground user session"
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_attention_restores_exact_original_work_identity() {
    let runtime_home = temp_path("busy-restore-identity");
    let selected = selected_master_with_workers(&["worker-alpha"]);
    bootstrap_runner_with_selected(&runtime_home, selected.clone());
    let task_id = seed_attention_required_task(&runtime_home, "worker-alpha");
    register_test_active_master_work(&runtime_home, "busy-parent-identity", "runtime-turn-46");
    update_master_active_work_safe_point(
        &runtime_home,
        &AgentId::new("master"),
        MasterWorkSafePoint::BeforeTerminalPersistence,
    )
    .expect("mark terminal safe point");
    let original = load_master_active_work(&runtime_home, &AgentId::new("master"))
        .expect("load active")
        .expect("active work");
    let action_task_id = task_id.clone();
    let runner = test_runner_with_selected(
        runtime_home.clone(),
        selected,
        Arc::new(StubMasterExecutor::new(move |request| {
            let runtime = TaskRuntime::boot(&request.runtime_home, AgentId::new("master"))
                .map_err(to_string)?;
            runtime
                .append_task(TaskAppendRequest {
                    task_id: action_task_id.clone(),
                    note: "attention_resolution: identity preserved".to_owned(),
                    actor: test_actor("master"),
                    watermark: test_watermark("busy-identity-append"),
                })
                .map_err(to_string)?;
            runtime
                .assign_task(freehand_task::TaskAssignRequest {
                    task_id: action_task_id.clone(),
                    agent_id: AgentId::new("worker-alpha"),
                    actor: test_actor("master"),
                    watermark: test_watermark("busy-identity-reassign"),
                })
                .map_err(to_string)?;
            Ok("identity restored".to_owned())
        })),
    );

    runner.run_once().expect("safe-point interrupt");
    let active = load_master_active_work(&runtime_home, &AgentId::new("master"))
        .expect("load active")
        .expect("active work");
    assert_eq!(active.work_id, original.work_id);
    assert_eq!(active.session_id, original.session_id);
    assert_eq!(active.logical_turn_id, original.logical_turn_id);
    assert_eq!(active.trace_id, original.trace_id);
    let resolution = active.attention_resolution.expect("resolution");
    assert_eq!(resolution.resume_from.work_id, original.work_id);
    assert_eq!(resolution.resume_from.session_id, original.session_id);
    assert_eq!(
        resolution.resume_from.logical_turn_id,
        original.logical_turn_id
    );
    assert_eq!(resolution.resume_from.trace_id, original.trace_id);
    assert_eq!(resolution.changed_task_ids, vec![task_id]);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_attention_cannot_restore_without_checkpoint() {
    let runtime_home = temp_path("busy-restore-missing-checkpoint");
    let selected = selected_master_with_workers(&["worker-alpha"]);
    bootstrap_runner_with_selected(&runtime_home, selected.clone());
    let task_id = seed_attention_required_task(&runtime_home, "worker-alpha");
    register_test_active_master_work(&runtime_home, "busy-parent-missing", "runtime-turn-47");
    update_master_active_work_safe_point(
        &runtime_home,
        &AgentId::new("master"),
        MasterWorkSafePoint::BeforeTerminalPersistence,
    )
    .expect("mark terminal safe point");
    let action_task_id = task_id.clone();
    let runner = test_runner_with_selected(
        runtime_home.clone(),
        selected,
        Arc::new(StubMasterExecutor::new(move |request| {
            clear_master_active_work_if_current(
                &request.runtime_home,
                &AgentId::new("master"),
                &TurnId::new("runtime-turn-47"),
            )
            .map_err(to_string)?;
            let runtime = TaskRuntime::boot(&request.runtime_home, AgentId::new("master"))
                .map_err(to_string)?;
            runtime
                .append_task(TaskAppendRequest {
                    task_id: action_task_id.clone(),
                    note: "attention_resolution: checkpoint removed".to_owned(),
                    actor: test_actor("master"),
                    watermark: test_watermark("busy-missing-checkpoint-append"),
                })
                .map_err(to_string)?;
            runtime
                .assign_task(freehand_task::TaskAssignRequest {
                    task_id: action_task_id.clone(),
                    agent_id: AgentId::new("worker-alpha"),
                    actor: test_actor("master"),
                    watermark: test_watermark("busy-missing-checkpoint-reassign"),
                })
                .map_err(to_string)?;
            Ok("checkpoint missing".to_owned())
        })),
    );

    let error = runner
        .run_once()
        .expect_err("missing active-work checkpoint must fail");
    assert!(
        error
            .to_string()
            .contains("cannot restore Master work without an active-work checkpoint")
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_resume_rejects_raw_worker_or_control_transcript() {
    let resolution = MasterAttentionResolution {
        attention_event_id: "attention-1".to_owned(),
        decision_kind: "task_advanced".to_owned(),
        changed_task_ids: vec![TaskId::new("task-1")],
        changed_constraints: vec!["raw_worker_transcript: hidden worker text".to_owned()],
        resume_from: MasterWorkReference {
            work_id: "work-1".to_owned(),
            session_id: SessionId::new("session-1"),
            logical_turn_id: TurnId::new("runtime-turn-1"),
            trace_id: TraceId::new("runtime-trace-1"),
        },
    };
    let error = validate_master_attention_resolution(&resolution)
        .expect_err("raw worker transcript must be rejected");
    assert!(error.contains("forbidden raw transcript/provider payload"));

    let provider_payload_resolution = MasterAttentionResolution {
        changed_constraints: vec!["provider_request_payload={...}".to_owned()],
        ..resolution
    };
    let error = validate_master_attention_resolution(&provider_payload_resolution)
        .expect_err("provider payload must be rejected");
    assert!(error.contains("forbidden raw transcript/provider payload"));
}

#[test]
fn production_master_resume_consumes_resolution_once() {
    let runtime_home = temp_path("resume-consumes-resolution-once");
    register_test_active_master_work(&runtime_home, "resume-once-session", "runtime-turn-88");
    let master_agent_id = AgentId::new("master");
    let mut active = load_master_active_work(&runtime_home, &master_agent_id)
        .expect("load active")
        .expect("active work");
    let resolution = MasterAttentionResolution {
        attention_event_id: "attention-once".to_owned(),
        decision_kind: "task_advanced".to_owned(),
        changed_task_ids: vec![TaskId::new("task-once")],
        changed_constraints: vec!["acceptance updated".to_owned()],
        resume_from: MasterWorkReference {
            work_id: active.work_id.clone(),
            session_id: active.session_id.clone(),
            logical_turn_id: active.logical_turn_id.clone(),
            trace_id: active.trace_id.clone(),
        },
    };
    active.attention_resolution = Some(resolution.clone());
    write_master_active_work_unlocked(&runtime_home, &active).expect("write resolution");

    let consumed = take_master_attention_resolution_if_current(
        &runtime_home,
        &master_agent_id,
        &SessionId::new("resume-once-session"),
        &TurnId::new("runtime-turn-88"),
        &TraceId::new("trace-runtime-turn-88"),
    )
    .expect("take resolution")
    .expect("resolution");
    assert_eq!(consumed, resolution);
    assert!(
        take_master_attention_resolution_if_current(
            &runtime_home,
            &master_agent_id,
            &SessionId::new("resume-once-session"),
            &TurnId::new("runtime-turn-88"),
            &TraceId::new("trace-runtime-turn-88"),
        )
        .expect("second take")
        .is_none(),
        "typed attention resolution must be consumed exactly once"
    );

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_resume_rejects_mismatched_return_identity() {
    let runtime_home = temp_path("resume-rejects-mismatched-return");
    register_test_active_master_work(&runtime_home, "resume-mismatch-session", "runtime-turn-89");
    let master_agent_id = AgentId::new("master");
    let mut active = load_master_active_work(&runtime_home, &master_agent_id)
        .expect("load active")
        .expect("active work");
    active.attention_resolution = Some(MasterAttentionResolution {
        attention_event_id: "attention-mismatch".to_owned(),
        decision_kind: "task_advanced".to_owned(),
        changed_task_ids: vec![TaskId::new("task-mismatch")],
        changed_constraints: Vec::new(),
        resume_from: MasterWorkReference {
            work_id: active.work_id.clone(),
            session_id: active.session_id.clone(),
            logical_turn_id: TurnId::new("runtime-turn-other"),
            trace_id: active.trace_id.clone(),
        },
    });
    write_master_active_work_unlocked(&runtime_home, &active).expect("write mismatched resolution");

    let error = take_master_attention_resolution_if_current(
        &runtime_home,
        &master_agent_id,
        &SessionId::new("resume-mismatch-session"),
        &TurnId::new("runtime-turn-89"),
        &TraceId::new("trace-runtime-turn-89"),
    )
    .expect_err("mismatched return identity must fail");
    assert!(error.contains("return identity mismatch"));

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
fn production_master_runner_fires_source_less_timer_in_internal_new_turn() {
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
    let observed_request = Arc::new(Mutex::new(None));
    let request_out = Arc::clone(&observed_request);
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        *request_out.lock().expect("request lock") = Some(request.clone());
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
    let request = observed_request
        .lock()
        .expect("request lock")
        .clone()
        .expect("request");
    assert!(request.session_id.as_str().starts_with("master-timer-"));
    assert!(request.turn_id.as_str().starts_with("master-timer-"));
    assert!(request.prompt.contains("Read TaskBoard and continue"));
    assert!(
        request
            .prompt
            .contains("new follow-up turn injected by a due timer")
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
fn production_master_runner_injects_due_timer_prompt_as_new_source_session_turn() {
    let runtime_home = temp_path("timer-source-session");
    bootstrap_runner(&runtime_home);
    let source_session_id = SessionId::new("visible-user-session");
    persist_parent_user_objective(
        &runtime_home,
        &source_session_id,
        "Wait for the timer, then inspect current Worker truth.",
    );
    let store = TimerStore::new(&runtime_home, &AgentId::new("master"));
    store
        .upsert_schedule(TimerSchedule {
            schema_version: 1,
            timer_id: "timer-source-session-proof".to_owned(),
            agent_id: AgentId::new("master"),
            status: "active".to_owned(),
            reason: "inject visible session prompt".to_owned(),
            prompt: "Inspect current truth.".to_owned(),
            next_due_at: now_unix_seconds().saturating_sub(1),
            created_at: now_unix_seconds().saturating_sub(10),
            updated_at: now_unix_seconds().saturating_sub(10),
            fired_count: 0,
            max_runs: 1,
            repeat: None,
            source_session_id: Some(source_session_id.clone()),
            source_turn_id: Some(TurnId::new("runtime-turn-1")),
            source_trace_id: Some(TraceId::new("parent-objective-trace")),
        })
        .expect("schedule timer");
    let observed = Arc::new(Mutex::new(None));
    let observed_out = Arc::clone(&observed);
    let runner = test_runner(
        runtime_home.clone(),
        Arc::new(StubMasterExecutor::new(move |request| {
            *observed_out.lock().expect("request lock") = Some(request.clone());
            Ok("visible wakeup".to_owned())
        })),
    );

    runner.run_once().expect("timer tick");
    let request = observed
        .lock()
        .expect("request lock")
        .clone()
        .expect("request");
    assert_eq!(request.session_id.as_str(), "visible-user-session");
    assert_eq!(request.turn_id.as_str(), "runtime-turn-2");
    assert_ne!(request.turn_id.as_str(), "runtime-turn-1");
    assert!(!request.session_id.as_str().starts_with("master-timer-"));
    assert!(
        request
            .prompt
            .contains("new follow-up turn injected by a due timer")
    );
    assert!(
        request
            .prompt
            .contains("not a resume or reopening of the source turn")
    );
    assert!(
        request
            .prompt
            .contains("Injected timer prompt:\nInspect current truth.")
    );
    fs::remove_dir_all(runtime_home).expect("cleanup");
}

#[test]
fn production_master_runner_resolves_chained_timer_to_original_session() {
    let runtime_home = temp_path("timer-source-chain");
    bootstrap_runner(&runtime_home);
    let store = TimerStore::new(&runtime_home, &AgentId::new("master"));
    let base = now_unix_seconds().saturating_sub(10);
    store
        .upsert_schedule(TimerSchedule {
            schema_version: 1,
            timer_id: "timer-origin".to_owned(),
            agent_id: AgentId::new("master"),
            status: "completed".to_owned(),
            reason: "origin".to_owned(),
            prompt: "origin".to_owned(),
            next_due_at: base,
            created_at: base,
            updated_at: base,
            fired_count: 1,
            max_runs: 1,
            repeat: None,
            source_session_id: Some(SessionId::new("original-user-session")),
            source_turn_id: None,
            source_trace_id: None,
        })
        .expect("origin timer");
    store
        .upsert_schedule(TimerSchedule {
            schema_version: 1,
            timer_id: "timer-chain".to_owned(),
            agent_id: AgentId::new("master"),
            status: "active".to_owned(),
            reason: "chain".to_owned(),
            prompt: "chain".to_owned(),
            next_due_at: now_unix_seconds().saturating_sub(1),
            created_at: base,
            updated_at: base,
            fired_count: 0,
            max_runs: 1,
            repeat: None,
            source_session_id: Some(SessionId::new("master-timer-timer-origin")),
            source_turn_id: None,
            source_trace_id: None,
        })
        .expect("chained timer");
    let observed_session = Arc::new(Mutex::new(None));
    let observed_out = Arc::clone(&observed_session);
    let runner = test_runner(
        runtime_home.clone(),
        Arc::new(StubMasterExecutor::new(move |request| {
            *observed_out.lock().expect("session lock") = Some(request.session_id.clone());
            Ok("chain wakeup".to_owned())
        })),
    );

    runner.run_once().expect("timer tick");
    assert_eq!(
        observed_session
            .lock()
            .expect("session lock")
            .as_ref()
            .map(SessionId::as_str),
        Some("original-user-session")
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
            .contains("new follow-up turn injected by a due timer")
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
fn production_master_runner_closed_loop_requires_next_round_before_final_evaluation() {
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
    let evaluation_calls = Arc::new(AtomicUsize::new(0));
    let evaluation_calls_out = Arc::clone(&evaluation_calls);
    let executor = Arc::new(StubMasterExecutor::new(move |request| {
        let call = evaluation_calls_out.fetch_add(1, Ordering::Relaxed);
        if !request
            .prompt
            .contains("create and assign the next required child tasks")
        {
            return Err("parent evaluation prompt forbids next-round work".to_owned());
        }
        if call == 0 {
            let runtime = TaskRuntime::boot(&request.runtime_home, AgentId::new("master"))
                .map_err(to_string)?;
            runtime
                .create_task(TaskCreateRequest {
                    task_id: Some(action_task_id.clone()),
                    title: "integrate accepted results".to_owned(),
                    content: "integrate alpha and beta into the requested final artifact"
                        .to_owned(),
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
        } else {
            Ok("overall goal verified only after integration closed".to_owned())
        }
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
    assert_eq!(evaluation_calls.load(Ordering::Relaxed), 1);
    assert_eq!(
        runner.run_once().expect("open next-round task tick"),
        ProductionMasterTickOutcome::Idle
    );
    assert_eq!(evaluation_calls.load(Ordering::Relaxed), 1);

    let execution_id = "exec-parent-integration".to_owned();
    runtime
        .claim_next_task(TaskClaimRequest {
            agent_id: AgentId::new("worker"),
            execution_id: execution_id.clone(),
            ttl_seconds: 300,
            actor: test_actor("worker"),
            watermark: test_watermark("integration-claim"),
        })
        .expect("claim integration");
    runtime
        .apply_execution_fact(ExecutionFact {
            execution_id,
            task_id: next_task_id.clone(),
            agent_id: AgentId::new("worker"),
            turn_id: Some(TurnId::new("worker-turn-integration")),
            occurred_at: now_unix_seconds(),
            kind: ExecutionFactKind::ReviewReady {
                summary: "integration review summary".to_owned(),
                deliverables: vec!["integrated-report.md".to_owned()],
                evidence: vec!["integration evidence".to_owned()],
            },
            watermark: test_watermark("integration-review-ready"),
        })
        .expect("submit integration review");
    runtime
        .approve_review(TaskMutationRequest {
            task_id: next_task_id.clone(),
            actor: test_actor("master"),
            watermark: test_watermark("integration-approve"),
        })
        .expect("approve integration");
    runtime
        .close_task(TaskMutationRequest {
            task_id: next_task_id,
            actor: test_actor("master"),
            watermark: test_watermark("integration-close"),
        })
        .expect("close integration");

    let final_outcome = runner.run_once().expect("final parent evaluation");
    assert!(
        matches!(
        final_outcome,
        ProductionMasterTickOutcome::ParentEvaluated { ref summary, .. }
            if summary == "overall goal verified only after integration closed"
        ),
        "unexpected final outcome: {final_outcome:?}"
    );
    assert_eq!(evaluation_calls.load(Ordering::Relaxed), 2);

    fs::remove_dir_all(runtime_home).expect("cleanup");
}

fn test_runner(
    runtime_home: PathBuf,
    executor: Arc<dyn MasterTurnExecutor>,
) -> ProductionMasterRunner {
    test_runner_with_selected(runtime_home, selected_master(), executor)
}

fn test_runner_with_selected(
    runtime_home: PathBuf,
    selected: SelectedAgentConfig,
    executor: Arc<dyn MasterTurnExecutor>,
) -> ProductionMasterRunner {
    ProductionMasterRunner::from_selected_agent_with_executor(selected, runtime_home, executor)
        .expect("master runner")
}

fn bootstrap_runner(runtime_home: &Path) {
    bootstrap_runner_with_selected(runtime_home, selected_master());
}

fn bootstrap_runner_with_selected(runtime_home: &Path, selected: SelectedAgentConfig) {
    let executor = Arc::new(StubMasterExecutor::new(|_| {
        Err("bootstrap must not execute".to_owned())
    }));
    let runner = test_runner_with_selected(runtime_home.to_path_buf(), selected, executor.clone());
    assert_eq!(
        runner.run_once().expect("bootstrap tick"),
        ProductionMasterTickOutcome::Idle
    );
    assert_eq!(executor.calls.load(Ordering::Relaxed), 0);
}

fn register_test_active_master_work(runtime_home: &Path, session_id: &str, turn_id: &str) {
    register_master_active_work(
        runtime_home,
        &AgentId::new("master"),
        &SessionId::new(session_id),
        &TurnId::new(turn_id),
        &TraceId::new(format!("trace-{turn_id}")),
    )
    .expect("register active Master work");
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

fn seed_interrupted_task(runtime_home: &Path, worker_id: &str) -> TaskId {
    seed_interrupted_task_with_parent(runtime_home, worker_id, None)
}

fn seed_interrupted_task_with_parent(
    runtime_home: &Path,
    worker_id: &str,
    parent_session_id: Option<SessionId>,
) -> TaskId {
    let runtime = TaskRuntime::boot(runtime_home, AgentId::new("master")).expect("runtime");
    let mut created_agents = Vec::new();
    for agent_id in ["worker-alpha", "worker-gamma", worker_id] {
        if created_agents.contains(&agent_id) {
            continue;
        }
        created_agents.push(agent_id);
        runtime
            .create_agent(AgentCreateRequest {
                agent_id: AgentId::new(agent_id),
                capabilities: vec!["workspace".to_owned(), "shell".to_owned()],
                actor: test_actor("master"),
                watermark: test_watermark("create-takeover-worker"),
            })
            .expect("create takeover worker");
    }
    let task_id = TaskId::new(format!("task-interrupted-{}", now_unix_nanos()));
    runtime
        .create_task(TaskCreateRequest {
            task_id: Some(task_id.clone()),
            title: "interrupted takeover task".to_owned(),
            content: "continue the same task after interruption".to_owned(),
            goal: "complete without duplicate task creation".to_owned(),
            deliverables: vec!["result.md".to_owned()],
            acceptance: vec!["same task id is preserved".to_owned()],
            priority: 90,
            target_cwd: Some(runtime_home.display().to_string()),
            dispatch: TaskDispatchRequest::Agent {
                agent_id: AgentId::new(worker_id),
            },
            parent: TaskParentRef {
                session_id: parent_session_id,
                turn_id: Some(TurnId::new("runtime-turn-parent")),
                trace_id: None,
            },
            actor: test_actor("master"),
            watermark: test_watermark("create-interrupted-task"),
        })
        .expect("create interrupted task");
    let execution_id = format!("exec-interrupted-{}", now_unix_nanos());
    runtime
        .claim_next_task(TaskClaimRequest {
            agent_id: AgentId::new(worker_id),
            execution_id: execution_id.clone(),
            ttl_seconds: 300,
            actor: test_actor(worker_id),
            watermark: test_watermark("claim-interrupted-task"),
        })
        .expect("claim interrupted task");
    runtime
        .apply_execution_fact(ExecutionFact {
            execution_id,
            task_id: task_id.clone(),
            agent_id: AgentId::new(worker_id),
            turn_id: Some(TurnId::new("worker-turn-interrupted")),
            occurred_at: now_unix_seconds(),
            kind: ExecutionFactKind::Interrupted {
                reason: "worker route interrupted; task remains schedulable".to_owned(),
                evidence: vec!["missing_or_expired_lease".to_owned()],
            },
            watermark: test_watermark("record-interrupted"),
        })
        .expect("record interrupted");
    task_id
}

fn seed_attention_required_task(runtime_home: &Path, worker_id: &str) -> TaskId {
    let runtime = TaskRuntime::boot(runtime_home, AgentId::new("master")).expect("runtime");
    runtime
        .create_agent(AgentCreateRequest {
            agent_id: AgentId::new(worker_id),
            capabilities: vec!["workspace".to_owned(), "shell".to_owned()],
            actor: test_actor("master"),
            watermark: test_watermark("create-attention-worker"),
        })
        .expect("create attention worker");
    let task_id = TaskId::new(format!("task-attention-{}", now_unix_nanos()));
    runtime
        .create_task(TaskCreateRequest {
            task_id: Some(task_id.clone()),
            title: "attention required task".to_owned(),
            content: "continue only if the task contract remains valid".to_owned(),
            goal: "complete after Master validates changed requirements".to_owned(),
            deliverables: vec!["result.md".to_owned()],
            acceptance: vec!["same task id is preserved".to_owned()],
            priority: 95,
            target_cwd: Some(runtime_home.display().to_string()),
            dispatch: TaskDispatchRequest::Agent {
                agent_id: AgentId::new(worker_id),
            },
            parent: TaskParentRef {
                session_id: None,
                turn_id: Some(TurnId::new("runtime-turn-attention")),
                trace_id: None,
            },
            actor: test_actor("master"),
            watermark: test_watermark("create-attention-task"),
        })
        .expect("create attention task");
    let execution_id = format!("exec-attention-{}", now_unix_nanos());
    runtime
        .claim_next_task(TaskClaimRequest {
            agent_id: AgentId::new(worker_id),
            execution_id: execution_id.clone(),
            ttl_seconds: 300,
            actor: test_actor(worker_id),
            watermark: test_watermark("claim-attention-task"),
        })
        .expect("claim attention task");
    runtime
        .apply_execution_fact(ExecutionFact {
            execution_id,
            task_id: task_id.clone(),
            agent_id: AgentId::new(worker_id),
            turn_id: Some(TurnId::new("worker-turn-attention")),
            occurred_at: now_unix_seconds(),
            kind: ExecutionFactKind::AttentionRequired {
                severity: "critical".to_owned(),
                change_kind: "task_contract_invalidated".to_owned(),
                reason: "new acceptance criteria conflict with current Worker plan".to_owned(),
                evidence: vec!["acceptance changed after execution started".to_owned()],
                proposed_adjustment: "revise acceptance and continue same task".to_owned(),
            },
            watermark: test_watermark("record-attention"),
        })
        .expect("record attention");
    task_id
}

fn selected_master() -> SelectedAgentConfig {
    selected_master_with_workers(&["worker"])
}

fn selected_master_with_workers(worker_ids: &[&str]) -> SelectedAgentConfig {
    SelectedAgentConfig {
        name: "master".to_owned(),
        mode: AgentMode::Master,
        node_id: "master-node".to_owned(),
        paired_agents: worker_ids
            .iter()
            .map(|worker_id| SelectedPeerAgentConfig {
                name: (*worker_id).to_owned(),
                mode: AgentMode::Slave,
                node_id: format!("{worker_id}-node"),
                allowed_pair_ip: None,
                pair_token_env: format!(
                    "FREEHAND_PAIR_TOKEN_{}",
                    worker_id.replace('-', "_").to_uppercase()
                ),
                provider_id: "worker-provider".to_owned(),
                fallback_provider_id: None,
            })
            .collect(),
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

fn test_attention_item(
    event_id: &str,
    kind: &str,
    severity: Option<&str>,
    task_priority: i64,
    admitted_sequence: u64,
) -> MasterAttentionItem {
    MasterAttentionItem {
        severity_rank: master_attention_severity_rank(&TaskEventInboxEntry {
            schema_version: 1,
            cursor: format!("cursor-{event_id}"),
            event_id: event_id.to_owned(),
            kind: kind.to_owned(),
            task_id: TaskId::new(format!("task-{event_id}")),
            execution_id: None,
            agent_id: None,
            created_at: admitted_sequence,
            payload: severity
                .map(|severity| serde_json::json!({ "severity": severity }))
                .unwrap_or(serde_json::Value::Null),
        }),
        task_priority,
        admitted_sequence,
        event: TaskEventInboxEntry {
            schema_version: 1,
            cursor: format!("cursor-{event_id}"),
            event_id: event_id.to_owned(),
            kind: kind.to_owned(),
            task_id: TaskId::new(format!("task-{event_id}")),
            execution_id: None,
            agent_id: None,
            created_at: admitted_sequence,
            payload: severity
                .map(|severity| serde_json::json!({ "severity": severity }))
                .unwrap_or(serde_json::Value::Null),
        },
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
