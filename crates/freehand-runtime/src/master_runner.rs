use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use freehand_config::{AgentMode, SelectedAgentConfig};
use freehand_contracts::{AgentId, SessionId, TerminalStatus, TraceId, TurnId};
use freehand_reason::{ReasonPersistence, ReasonPersistenceError};
use freehand_task::{
    TaskActor, TaskAppendRequest, TaskBoardQuery, TaskEventInboxEntry, TaskEventInboxQuery, TaskId,
    TaskRuntime, TaskSnapshot, TaskStatus, TaskWatermark,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    DueTimerSchedule, LiveReasonTaskDecisionBoundary, LiveReasonTaskDecisionMode,
    LiveReasonTurnRequest, RuntimeAgentBootstrapError, claim_due_timer_schedule,
    complete_due_timer_schedule, fail_due_timer_schedule, load_default_runtime_agent,
    now_unix_seconds, run_live_reason_turn, run_master_lifecycle_reason_turn,
    runtime_turn_position, sanitize_identifier, ui_user_text_for_turn,
};

#[cfg(test)]
mod tests;

const DEFAULT_POLL_INTERVAL_MILLIS: u64 = 1_000;
const MASTER_RETRY_INITIAL_BACKOFF_MILLIS: u64 = 1_000;
const MASTER_RETRY_MAX_BACKOFF_MILLIS: u64 = 30_000;
const MASTER_LIFECYCLE_DECISION_MAX_ROUNDS: usize = 8;
const MASTER_BLOCKED_DECISION_AUTO_APPEND_ATTEMPTS: u32 = 16;
const CANCEL_POLL_MILLIS: u64 = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductionMasterTickOutcome {
    Idle,
    TaskAdvanced {
        task_id: TaskId,
        from: TaskStatus,
        to: TaskStatus,
        summary: String,
    },
    BlockedObserved {
        task_id: TaskId,
        summary: String,
    },
    TimerFired {
        timer_id: String,
        summary: String,
    },
    ParentEvaluated {
        parent_session_id: SessionId,
        evaluated_child_task_ids: Vec<TaskId>,
        summary: String,
    },
}

#[derive(Debug, Error)]
pub enum ProductionMasterRunnerError {
    #[error("master bootstrap failed: {0}")]
    Bootstrap(#[from] RuntimeAgentBootstrapError),
    #[error("master lifecycle runner requires a master agent, got `{mode}` for `{agent_name}`")]
    RequiresMasterMode { agent_name: String, mode: String },
    #[error("master Task Center failed: {0}")]
    TaskCenter(String),
    #[error("master lifecycle state failed: {0}")]
    State(String),
    #[error("master lifecycle turn failed: {0}")]
    Execution(String),
    #[error("master made no task decision for review-ready task `{task_id}`")]
    MissingReviewDecision { task_id: String },
    #[error("master left approved review-ready task `{task_id}` without closing it")]
    IncompleteReviewDecision { task_id: String },
    #[error("master made no persisted blocked-task decision for `{task_id}`")]
    MissingBlockedDecision { task_id: String },
    #[error("master made no recovery decision for interrupted task `{task_id}`")]
    MissingInterruptedDecision { task_id: String },
}

impl ProductionMasterRunnerError {
    fn is_retryable_lifecycle_failure(&self) -> bool {
        matches!(
            self,
            Self::Execution(_)
                | Self::MissingReviewDecision { .. }
                | Self::IncompleteReviewDecision { .. }
                | Self::MissingBlockedDecision { .. }
                | Self::MissingInterruptedDecision { .. }
        )
    }
}

#[derive(Debug, Clone, Copy)]
struct MasterLoopRetryPolicy {
    initial_backoff: Duration,
    max_backoff: Duration,
}

impl Default for MasterLoopRetryPolicy {
    fn default() -> Self {
        Self {
            initial_backoff: Duration::from_millis(MASTER_RETRY_INITIAL_BACKOFF_MILLIS),
            max_backoff: Duration::from_millis(MASTER_RETRY_MAX_BACKOFF_MILLIS),
        }
    }
}

impl MasterLoopRetryPolicy {
    fn delay(self, consecutive_failures: u32) -> Duration {
        let exponent = consecutive_failures.saturating_sub(1).min(31);
        let multiplier = 1_u32 << exponent;
        self.initial_backoff
            .saturating_mul(multiplier)
            .min(self.max_backoff)
    }
}

trait MasterTurnExecutor: Send + Sync {
    fn execute(
        &self,
        selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
        decision_boundary: LiveReasonTaskDecisionBoundary,
    ) -> Result<String, String>;

    fn execute_timer(
        &self,
        selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
    ) -> Result<String, String>;

    fn execute_parent_evaluation(
        &self,
        selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
    ) -> Result<String, String>;
}

struct LiveMasterTurnExecutor;

impl MasterTurnExecutor for LiveMasterTurnExecutor {
    fn execute(
        &self,
        selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
        decision_boundary: LiveReasonTaskDecisionBoundary,
    ) -> Result<String, String> {
        let outcome = run_master_lifecycle_reason_turn(selected, request, decision_boundary)
            .map_err(|error| error.to_string())?;
        let terminal = outcome
            .turn
            .terminal_event
            .as_ref()
            .ok_or_else(|| "master lifecycle turn closed without terminal event".to_owned())?;
        Ok(terminal.summary.clone())
    }

    fn execute_timer(
        &self,
        selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
    ) -> Result<String, String> {
        let outcome = run_live_reason_turn(selected, request).map_err(|error| error.to_string())?;
        let terminal = outcome
            .turn
            .terminal_event
            .as_ref()
            .ok_or_else(|| "timer wakeup turn closed without terminal event".to_owned())?;
        Ok(terminal.summary.clone())
    }

    fn execute_parent_evaluation(
        &self,
        selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
    ) -> Result<String, String> {
        let outcome = run_live_reason_turn(selected, request).map_err(|error| error.to_string())?;
        let terminal = outcome
            .turn
            .terminal_event
            .as_ref()
            .ok_or_else(|| "parent evaluation turn closed without terminal event".to_owned())?;
        Ok(terminal.summary.clone())
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct MasterLoopState {
    #[serde(default)]
    initialized: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cursor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    retry_event_id: Option<String>,
    #[serde(default)]
    retry_attempt: u32,
    #[serde(default)]
    completed_parent_evaluations: BTreeSet<String>,
}

pub struct ProductionMasterRunner {
    selected: SelectedAgentConfig,
    runtime_home: PathBuf,
    master_agent_id: AgentId,
    executor: Arc<dyn MasterTurnExecutor>,
}

impl ProductionMasterRunner {
    pub fn from_default_config(agent_name: &str) -> Result<Self, ProductionMasterRunnerError> {
        let bootstrap = load_default_runtime_agent(agent_name)?;
        Self::from_selected_agent(bootstrap.selected_agent, bootstrap.runtime_home)
    }

    pub fn from_selected_agent(
        selected: SelectedAgentConfig,
        runtime_home: PathBuf,
    ) -> Result<Self, ProductionMasterRunnerError> {
        Self::from_selected_agent_with_executor(
            selected,
            runtime_home,
            Arc::new(LiveMasterTurnExecutor),
        )
    }

    fn from_selected_agent_with_executor(
        selected: SelectedAgentConfig,
        runtime_home: PathBuf,
        executor: Arc<dyn MasterTurnExecutor>,
    ) -> Result<Self, ProductionMasterRunnerError> {
        if selected.mode != AgentMode::Master {
            return Err(ProductionMasterRunnerError::RequiresMasterMode {
                agent_name: selected.name.clone(),
                mode: selected.mode.as_str().to_owned(),
            });
        }
        fs::create_dir_all(&runtime_home)
            .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
        Ok(Self {
            master_agent_id: AgentId::new(selected.name.clone()),
            selected,
            runtime_home,
            executor,
        })
    }

    pub fn run_once(&self) -> Result<ProductionMasterTickOutcome, ProductionMasterRunnerError> {
        let task_runtime = self.open_task_center()?;
        let mut state = self.load_state()?;
        if !state.initialized {
            let inbox = task_runtime
                .query_event_inbox(TaskEventInboxQuery {
                    after_cursor: None,
                    limit: usize::MAX,
                })
                .map_err(task_center_error)?;
            state.initialized = true;
            state.cursor = inbox.next_cursor;
            self.write_state(&state)?;
            return Ok(ProductionMasterTickOutcome::Idle);
        }

        let inbox = task_runtime
            .query_event_inbox(TaskEventInboxQuery {
                after_cursor: state.cursor.clone(),
                limit: usize::MAX,
            })
            .map_err(task_center_error)?;
        if inbox.events.is_empty() {
            if let Some(outcome) = self.handle_due_timer()? {
                return Ok(outcome);
            }
            return Ok(ProductionMasterTickOutcome::Idle);
        }

        let mut latest_outcome = ProductionMasterTickOutcome::Idle;
        for event in inbox.events {
            let attempt = if state.retry_event_id.as_deref() == Some(event.event_id.as_str()) {
                state.retry_attempt
            } else {
                0
            };
            match self.handle_event(&task_runtime, &event, attempt, &mut state) {
                Ok(Some(outcome)) => latest_outcome = outcome,
                Ok(None) => {}
                Err(error) => {
                    if error.is_retryable_lifecycle_failure() {
                        state.retry_event_id = Some(event.event_id.clone());
                        state.retry_attempt = attempt.saturating_add(1);
                        self.write_state(&state)?;
                    }
                    return Err(error);
                }
            }
            state.retry_event_id = None;
            state.retry_attempt = 0;
            state.cursor = Some(event.cursor);
            self.write_state(&state)?;
        }
        if latest_outcome != ProductionMasterTickOutcome::Idle {
            return Ok(latest_outcome);
        }
        if let Some(outcome) = self.handle_due_timer()? {
            return Ok(outcome);
        }
        Ok(latest_outcome)
    }

    fn handle_due_timer(
        &self,
    ) -> Result<Option<ProductionMasterTickOutcome>, ProductionMasterRunnerError> {
        let Some(due) = claim_due_timer_schedule(
            &self.runtime_home,
            &self.master_agent_id,
            now_unix_seconds(),
        )
        .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?
        else {
            return Ok(None);
        };
        let summary = match self.executor.execute_timer(
            &self.selected,
            timer_live_request(&self.runtime_home, &due)?,
        ) {
            Ok(summary) => summary,
            Err(error) => {
                fail_due_timer_schedule(&self.runtime_home, &self.master_agent_id, &due, &error)
                    .map_err(|state_error| {
                        ProductionMasterRunnerError::State(state_error.to_string())
                    })?;
                return Err(ProductionMasterRunnerError::Execution(error));
            }
        };
        complete_due_timer_schedule(&self.runtime_home, &self.master_agent_id, &due)
            .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
        Ok(Some(ProductionMasterTickOutcome::TimerFired {
            timer_id: due.schedule.timer_id,
            summary,
        }))
    }

    pub fn run(&self) -> Result<(), ProductionMasterRunnerError> {
        self.run_until(Arc::new(AtomicBool::new(false)))
    }

    pub fn run_until(&self, cancel: Arc<AtomicBool>) -> Result<(), ProductionMasterRunnerError> {
        self.run_until_with_policy(cancel, MasterLoopRetryPolicy::default())
    }

    fn run_until_with_policy(
        &self,
        cancel: Arc<AtomicBool>,
        retry_policy: MasterLoopRetryPolicy,
    ) -> Result<(), ProductionMasterRunnerError> {
        let interval = Duration::from_millis(DEFAULT_POLL_INTERVAL_MILLIS);
        let mut consecutive_retryable_failures = 0_u32;
        while !cancel.load(Ordering::Acquire) {
            match self.run_once() {
                Ok(ProductionMasterTickOutcome::Idle) => {
                    consecutive_retryable_failures = 0;
                }
                Ok(outcome) => {
                    consecutive_retryable_failures = 0;
                    println!("master lifecycle runner outcome: {outcome:?}");
                }
                Err(error) if error.is_retryable_lifecycle_failure() => {
                    consecutive_retryable_failures =
                        consecutive_retryable_failures.saturating_add(1);
                    let delay = retry_policy.delay(consecutive_retryable_failures);
                    eprintln!(
                        "master lifecycle decision retry attempt={} retry_in_ms={} error={error}",
                        consecutive_retryable_failures,
                        delay.as_millis()
                    );
                    sleep_with_cancel(&cancel, delay);
                    continue;
                }
                Err(error) => return Err(error),
            }
            sleep_with_cancel(&cancel, interval);
        }
        Ok(())
    }

    fn handle_event(
        &self,
        task_runtime: &TaskRuntime,
        event: &TaskEventInboxEntry,
        attempt: u32,
        state: &mut MasterLoopState,
    ) -> Result<Option<ProductionMasterTickOutcome>, ProductionMasterRunnerError> {
        let task = task_runtime
            .query_task(&event.task_id)
            .map_err(task_center_error)?;
        if event.kind == "task_closed" {
            return self.handle_parent_task_closed(task_runtime, &task, attempt, state);
        }
        let actionable = match event.kind.as_str() {
            "review_ready" => matches!(
                task.status,
                TaskStatus::ReviewSubmitted | TaskStatus::Approved
            ),
            "execution_blocked" => task.status == TaskStatus::Blocked,
            "execution_interrupted" => task.status == TaskStatus::Interrupted,
            _ => false,
        };
        if !actionable {
            return Ok(None);
        }

        let from = task.status.clone();
        if event.kind == "execution_blocked"
            && from == TaskStatus::Blocked
            && attempt >= MASTER_BLOCKED_DECISION_AUTO_APPEND_ATTEMPTS
        {
            let note = format!(
                "blocked_decision: Master lifecycle provider remained unavailable after {attempt} attempts; leaving task blocked and continuing other pending lifecycle events"
            );
            task_runtime
                .append_task(TaskAppendRequest {
                    task_id: task.task_id.clone(),
                    note: note.clone(),
                    actor: master_loop_actor(&self.master_agent_id),
                    watermark: master_loop_watermark("blocked_auto_append"),
                })
                .map_err(task_center_error)?;
            return Ok(Some(ProductionMasterTickOutcome::BlockedObserved {
                task_id: task.task_id,
                summary: note,
            }));
        }
        let history_len_before = task_runtime
            .task_history(&task.task_id)
            .map_err(task_center_error)?
            .len();
        let summary = self
            .executor
            .execute(
                &self.selected,
                master_live_request(
                    &self.runtime_home,
                    &self.selected.paired_agent_name,
                    &task,
                    event,
                    attempt,
                )?,
                master_decision_boundary(&task),
            )
            .map_err(ProductionMasterRunnerError::Execution)?;
        let current_runtime = self.open_task_center()?;
        let current = current_runtime
            .query_task(&task.task_id)
            .map_err(task_center_error)?;

        if matches!(from, TaskStatus::ReviewSubmitted | TaskStatus::Approved) {
            match current.status {
                TaskStatus::Rejected | TaskStatus::Closed => {}
                TaskStatus::Approved => {
                    return Err(ProductionMasterRunnerError::IncompleteReviewDecision {
                        task_id: task.task_id.as_str().to_owned(),
                    });
                }
                _ => {
                    return Err(ProductionMasterRunnerError::MissingReviewDecision {
                        task_id: task.task_id.as_str().to_owned(),
                    });
                }
            }
        }
        if from == TaskStatus::Blocked && current.status == TaskStatus::Blocked {
            let history = current_runtime
                .task_history(&task.task_id)
                .map_err(task_center_error)?;
            let persisted_decision = history.len() > history_len_before
                && history
                    .last()
                    .map(|event| {
                        event.event_type == "TaskProgressed"
                            && event.actor.agent_id == self.master_agent_id
                    })
                    .unwrap_or(false);
            if !persisted_decision {
                return Err(ProductionMasterRunnerError::MissingBlockedDecision {
                    task_id: task.task_id.as_str().to_owned(),
                });
            }
            return Ok(Some(ProductionMasterTickOutcome::BlockedObserved {
                task_id: task.task_id,
                summary,
            }));
        }
        if from == TaskStatus::Interrupted && current.status == TaskStatus::Interrupted {
            return Err(ProductionMasterRunnerError::MissingInterruptedDecision {
                task_id: task.task_id.as_str().to_owned(),
            });
        }
        Ok(Some(ProductionMasterTickOutcome::TaskAdvanced {
            task_id: task.task_id,
            from,
            to: current.status,
            summary,
        }))
    }

    fn handle_parent_task_closed(
        &self,
        task_runtime: &TaskRuntime,
        closed_task: &TaskSnapshot,
        attempt: u32,
        state: &mut MasterLoopState,
    ) -> Result<Option<ProductionMasterTickOutcome>, ProductionMasterRunnerError> {
        if closed_task.status != TaskStatus::Closed {
            return Ok(None);
        }
        let Some(parent_session_id) = closed_task.parent.session_id.clone() else {
            return Ok(None);
        };
        let board = task_runtime
            .query_task_board(TaskBoardQuery {
                status: None,
                assignee: None,
                include_terminal: true,
            })
            .map_err(task_center_error)?;
        let mut children = board
            .tasks
            .into_iter()
            .filter(|task| task.parent.session_id.as_ref() == Some(&parent_session_id))
            .collect::<Vec<_>>();
        if children.is_empty()
            || children
                .iter()
                .any(|task| task.status != TaskStatus::Closed)
        {
            return Ok(None);
        }
        children.sort_by(|left, right| left.task_id.cmp(&right.task_id));
        let evaluation_key = parent_evaluation_key(&parent_session_id, &children);
        if state.completed_parent_evaluations.contains(&evaluation_key) {
            return Ok(None);
        }
        let evaluation_marker = parent_evaluation_marker(&evaluation_key);
        if let Some(summary) = persisted_parent_evaluation_summary(
            &self.runtime_home,
            &self.master_agent_id,
            &parent_session_id,
            &evaluation_marker,
        )? {
            state.completed_parent_evaluations.insert(evaluation_key);
            return Ok(Some(ProductionMasterTickOutcome::ParentEvaluated {
                parent_session_id,
                evaluated_child_task_ids: children.into_iter().map(|task| task.task_id).collect(),
                summary,
            }));
        }
        let user_objectives = parent_user_objectives(
            &self.runtime_home,
            &self.master_agent_id,
            &parent_session_id,
        )?;
        let completed_subtasks = children
            .iter()
            .map(|task| parent_completed_subtask_truth(task_runtime, task))
            .collect::<Result<Vec<_>, _>>()?;
        let evaluation_turn_id = next_parent_evaluation_turn_id(
            &self.runtime_home,
            &self.master_agent_id,
            &parent_session_id,
        )?;
        let request = parent_evaluation_live_request(
            &self.runtime_home,
            &parent_session_id,
            &evaluation_turn_id,
            &evaluation_marker,
            attempt,
            &user_objectives,
            &completed_subtasks,
        )?;
        let summary = self
            .executor
            .execute_parent_evaluation(&self.selected, request)
            .map_err(ProductionMasterRunnerError::Execution)?;
        state.completed_parent_evaluations.insert(evaluation_key);
        Ok(Some(ProductionMasterTickOutcome::ParentEvaluated {
            parent_session_id,
            evaluated_child_task_ids: children.into_iter().map(|task| task.task_id).collect(),
            summary,
        }))
    }

    fn open_task_center(&self) -> Result<TaskRuntime, ProductionMasterRunnerError> {
        TaskRuntime::boot(&self.runtime_home, self.master_agent_id.clone())
            .map_err(task_center_error)
    }

    fn state_path(&self) -> PathBuf {
        self.runtime_home
            .join("state")
            .join("master-loop")
            .join(format!("{}.json", self.master_agent_id.as_str()))
    }

    fn load_state(&self) -> Result<MasterLoopState, ProductionMasterRunnerError> {
        let path = self.state_path();
        if !path.exists() {
            return Ok(MasterLoopState::default());
        }
        let raw = fs::read_to_string(&path)
            .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
        serde_json::from_str(&raw)
            .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))
    }

    fn write_state(&self, state: &MasterLoopState) -> Result<(), ProductionMasterRunnerError> {
        let path = self.state_path();
        let parent = path.parent().ok_or_else(|| {
            ProductionMasterRunnerError::State("master loop state path has no parent".to_owned())
        })?;
        fs::create_dir_all(parent)
            .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
        let temp = path.with_extension("tmp");
        let raw = serde_json::to_string_pretty(state)
            .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
        fs::write(&temp, raw)
            .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
        fs::rename(&temp, &path)
            .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))
    }
}

fn master_live_request(
    runtime_home: &Path,
    worker_name: &str,
    task: &TaskSnapshot,
    event: &TaskEventInboxEntry,
    attempt: u32,
) -> Result<LiveReasonTurnRequest, ProductionMasterRunnerError> {
    let task_json = serde_json::to_string_pretty(task)
        .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
    let event_json = serde_json::to_string_pretty(event)
        .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
    let event_key = sanitize_identifier(&event.event_id);
    let task_key = sanitize_identifier(task.task_id.as_str());
    let lifecycle_session_id = format!("master-lifecycle-{task_key}");
    Ok(LiveReasonTurnRequest {
        runtime_home: runtime_home.to_path_buf(),
        session_id: SessionId::new(lifecycle_session_id),
        turn_id: TurnId::new(format!(
            "master-lifecycle-{event_key}-attempt-{attempt}-decision"
        )),
        trace_id: TraceId::new(format!(
            "master-lifecycle-trace-{event_key}-attempt-{attempt}"
        )),
        prompt: format!(
            "You are the production Master lifecycle coordinator.\n\
Use the task tool and Task Center truth; do not answer with prose-only status.\n\
Configured Worker: {worker_name}\n\
\n\
Rules:\n\
- review_ready: query/history, then reject with concrete requirements or approve and close. Approved is not terminal; close it in the same lifecycle decision.\n\
- execution_blocked: inspect the blocker. Reassign only when retry is justified; otherwise call task(op=\"append\", task_id=<task-id>, note=\"blocked_decision: <required external action>\") to persist why it remains blocked.\n\
- execution_interrupted: assign the task back to the configured Worker for a new execution.\n\
- one trigger event owns one decision turn. After the required Task Center mutation is persisted, stop; never wait inside this turn for a future Worker event.\n\
- never fabricate completion, approval, evidence, or task state.\n\
\n\
Task snapshot:\n{task_json}\n\
\n\
Trigger event:\n{event_json}"
        ),
        cwd: Some(runtime_home.to_path_buf()),
        stream: false,
        cancel_token: None,
    })
}

#[derive(Debug, Serialize)]
struct ParentCompletedSubtaskTruth {
    task_id: TaskId,
    title: String,
    content: String,
    goal: String,
    required_deliverables: Vec<String>,
    acceptance: Vec<String>,
    review_summary: String,
    review_deliverables: Vec<String>,
    review_evidence: Vec<String>,
}

fn parent_completed_subtask_truth(
    task_runtime: &TaskRuntime,
    task: &TaskSnapshot,
) -> Result<ParentCompletedSubtaskTruth, ProductionMasterRunnerError> {
    let history = task_runtime
        .task_history(&task.task_id)
        .map_err(task_center_error)?;
    let review = history
        .iter()
        .rev()
        .find(|event| event.event_type == "TaskReviewSubmitted")
        .ok_or_else(|| {
            ProductionMasterRunnerError::State(format!(
                "closed child task `{}` has no TaskReviewSubmitted truth",
                task.task_id.as_str()
            ))
        })?;
    let summary = review
        .payload
        .get("summary")
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            ProductionMasterRunnerError::State(format!(
                "closed child task `{}` review summary is missing",
                task.task_id.as_str()
            ))
        })?
        .to_owned();
    Ok(ParentCompletedSubtaskTruth {
        task_id: task.task_id.clone(),
        title: task.title.clone(),
        content: task.content.clone(),
        goal: task.goal.clone(),
        required_deliverables: task.deliverables.clone(),
        acceptance: task.acceptance.clone(),
        review_summary: summary,
        review_deliverables: review_payload_strings(&review.payload, "deliverables"),
        review_evidence: review_payload_strings(&review.payload, "evidence"),
    })
}

fn review_payload_strings(payload: &serde_json::Value, field: &str) -> Vec<String> {
    payload
        .get(field)
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_owned)
        .collect()
}

fn parent_evaluation_key(parent_session_id: &SessionId, children: &[TaskSnapshot]) -> String {
    format!(
        "{}|{}",
        parent_session_id.as_str(),
        children
            .iter()
            .map(|task| format!("{}:{}", task.task_id.as_str(), task.last_event_seq))
            .collect::<Vec<_>>()
            .join(",")
    )
}

fn parent_evaluation_marker(evaluation_key: &str) -> String {
    format!("{:016x}", stable_parent_evaluation_hash(evaluation_key))
}

fn stable_parent_evaluation_hash(value: &str) -> u64 {
    value
        .as_bytes()
        .iter()
        .fold(0xcbf29ce484222325, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        })
}

fn persisted_parent_evaluation_summary(
    runtime_home: &Path,
    agent_id: &AgentId,
    parent_session_id: &SessionId,
    evaluation_marker: &str,
) -> Result<Option<String>, ProductionMasterRunnerError> {
    let persistence = ReasonPersistence::new(runtime_home.to_path_buf(), agent_id.clone());
    let turns = match persistence.restore_turn_snapshots_for_ui(parent_session_id) {
        Ok(turns) => turns,
        Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => return Ok(None),
        Err(error) => return Err(ProductionMasterRunnerError::State(error.to_string())),
    };
    Ok(turns.into_iter().find_map(|turn| {
        let terminal = turn.terminal_event?;
        (turn.request.user_text.contains(&format!(
            "<freehand_parent_evaluation id=\"{evaluation_marker}\">"
        )) && matches!(
            terminal.status,
            TerminalStatus::Success | TerminalStatus::ToolPending | TerminalStatus::Blocked
        ))
        .then_some(terminal.summary)
    }))
}

fn next_parent_evaluation_turn_id(
    runtime_home: &Path,
    agent_id: &AgentId,
    parent_session_id: &SessionId,
) -> Result<TurnId, ProductionMasterRunnerError> {
    let persistence = ReasonPersistence::new(runtime_home.to_path_buf(), agent_id.clone());
    let turns = match persistence.restore_turn_snapshots_for_ui(parent_session_id) {
        Ok(turns) => turns,
        Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => Vec::new(),
        Err(error) => return Err(ProductionMasterRunnerError::State(error.to_string())),
    };
    let next = turns
        .iter()
        .map(|turn| runtime_turn_position(&turn.request.turn_id).0)
        .max()
        .unwrap_or(0)
        .saturating_add(1);
    Ok(TurnId::new(format!("runtime-turn-{next}")))
}

fn parent_evaluation_live_request(
    runtime_home: &Path,
    parent_session_id: &SessionId,
    turn_id: &TurnId,
    evaluation_marker: &str,
    attempt: u32,
    user_objectives: &[String],
    completed_subtasks: &[ParentCompletedSubtaskTruth],
) -> Result<LiveReasonTurnRequest, ProductionMasterRunnerError> {
    let objectives_json = serde_json::to_string_pretty(user_objectives)
        .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
    let subtasks_json = serde_json::to_string_pretty(completed_subtasks)
        .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
    Ok(LiveReasonTurnRequest {
        runtime_home: runtime_home.to_path_buf(),
        session_id: parent_session_id.clone(),
        turn_id: turn_id.clone(),
        trace_id: TraceId::new(format!(
            "master-parent-evaluate-trace-{evaluation_marker}-attempt-{attempt}"
        )),
        prompt: format!(
            "<freehand_parent_evaluation id=\"{evaluation_marker}\">\n\
You are the production Master resuming the original user session after the current set of required child Worker tasks closed.\n\
This is an overall-goal evaluation turn, not a result aggregation turn.\n\
Compare the original user objective history with every decomposed child task's content, goal, required deliverables, acceptance criteria, and accepted Worker review truth.\n\
Do not expose raw Worker transcripts or internal lifecycle session text.\n\
\n\
Decision contract:\n\
- If accepted child work is insufficient, inconsistent, or needs improvement, use the task tool to create and assign concrete correction/improvement child tasks in this same parent session.\n\
- If the completed subgoals reveal additional work needed for the overall objective, create and assign the next required child tasks.\n\
- If an external dependency prevents progress, return an explicit blocked decision naming the required action.\n\
- Use `claim=\"complete\"` and produce the final user-visible answer only when the overall user objective is actually verified complete.\n\
- Do not merely summarize the Worker results and call that completion.\n\
\n\
Original user objective history:\n\
{objectives_json}\n\
\n\
Completed subtask and accepted review truth:\n{subtasks_json}"
        ),
        cwd: Some(runtime_home.to_path_buf()),
        stream: false,
        cancel_token: None,
    })
}

fn parent_user_objectives(
    runtime_home: &Path,
    agent_id: &AgentId,
    parent_session_id: &SessionId,
) -> Result<Vec<String>, ProductionMasterRunnerError> {
    let persistence = ReasonPersistence::new(runtime_home.to_path_buf(), agent_id.clone());
    let turns = match persistence.restore_turn_snapshots_for_ui(parent_session_id) {
        Ok(turns) => turns,
        Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => Vec::new(),
        Err(error) => return Err(ProductionMasterRunnerError::State(error.to_string())),
    };
    let mut seen = BTreeSet::new();
    let objectives = turns
        .into_iter()
        .filter(|turn| runtime_turn_position(&turn.request.turn_id).1 == 1)
        .map(|turn| ui_user_text_for_turn(&turn))
        .filter(|text| !text.trim().is_empty())
        .filter(|text| !text.contains("<freehand_parent_"))
        .filter(|text| seen.insert(text.clone()))
        .collect::<Vec<_>>();
    if objectives.is_empty() {
        return Err(ProductionMasterRunnerError::State(format!(
            "parent session `{}` has no persisted user objective truth",
            parent_session_id.as_str()
        )));
    }
    Ok(objectives)
}

fn timer_live_request(
    runtime_home: &Path,
    due: &DueTimerSchedule,
) -> Result<LiveReasonTurnRequest, ProductionMasterRunnerError> {
    let timer_json = serde_json::to_string_pretty(&due.schedule)
        .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
    let timer_key = sanitize_identifier(&due.schedule.timer_id);
    Ok(LiveReasonTurnRequest {
        runtime_home: runtime_home.to_path_buf(),
        session_id: SessionId::new(format!("master-timer-{timer_key}")),
        turn_id: TurnId::new(format!("master-timer-{timer_key}-fire-{}", due.fired_at)),
        trace_id: TraceId::new(format!(
            "master-timer-trace-{timer_key}-fire-{}",
            due.fired_at
        )),
        prompt: format!(
            "You are the production Master resumed by an internal timer.\n\
Use current framework truth and the timer wakeup prompt; do not assume task state from memory.\n\
\n\
Timer schedule truth:\n{timer_json}\n\
\n\
Wakeup prompt:\n{}",
            due.schedule.prompt
        ),
        cwd: Some(runtime_home.to_path_buf()),
        stream: false,
        cancel_token: None,
    })
}

fn master_decision_boundary(task: &TaskSnapshot) -> LiveReasonTaskDecisionBoundary {
    let mode = match task.status {
        TaskStatus::ReviewSubmitted | TaskStatus::Approved => {
            LiveReasonTaskDecisionMode::TargetStatuses(vec![
                TaskStatus::Rejected,
                TaskStatus::Closed,
            ])
        }
        TaskStatus::Blocked | TaskStatus::Interrupted => LiveReasonTaskDecisionMode::TargetMutation,
        _ => LiveReasonTaskDecisionMode::TargetMutation,
    };
    LiveReasonTaskDecisionBoundary {
        task_id: task.task_id.clone(),
        initial_event_seq: task.last_event_seq,
        mode,
        max_rounds: MASTER_LIFECYCLE_DECISION_MAX_ROUNDS,
    }
}

fn task_center_error(error: freehand_task::TaskError) -> ProductionMasterRunnerError {
    ProductionMasterRunnerError::TaskCenter(error.to_string())
}

fn master_loop_actor(agent_id: &AgentId) -> TaskActor {
    TaskActor {
        agent_id: agent_id.clone(),
        source: "runtime.master-worker-loop".to_owned(),
        session_id: None,
        turn_id: None,
        trace_id: None,
    }
}

fn master_loop_watermark(hook: &str) -> TaskWatermark {
    TaskWatermark {
        metadata_id: None,
        hook: Some(format!("runtime.master-worker-loop.{hook}")),
        action_tool_call_id: None,
    }
}

fn sleep_with_cancel(cancel: &AtomicBool, duration: Duration) {
    let poll = Duration::from_millis(CANCEL_POLL_MILLIS);
    let mut remaining = duration;
    while !remaining.is_zero() && !cancel.load(Ordering::Acquire) {
        let step = remaining.min(poll);
        thread::sleep(step);
        remaining = remaining.saturating_sub(step);
    }
}
