use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use freehand_config::{AgentMode, SelectedAgentConfig};
use freehand_contracts::{AgentId, SessionId, TraceId, TurnId};
use freehand_task::{
    TaskEventInboxEntry, TaskEventInboxQuery, TaskId, TaskRuntime, TaskSnapshot, TaskStatus,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    LiveReasonTaskDecisionBoundary, LiveReasonTaskDecisionMode, LiveReasonTurnRequest,
    RuntimeAgentBootstrapError, load_default_runtime_agent, run_master_lifecycle_reason_turn,
    sanitize_identifier,
};

#[cfg(test)]
mod tests;

const DEFAULT_POLL_INTERVAL_MILLIS: u64 = 1_000;
const MASTER_RETRY_INITIAL_BACKOFF_MILLIS: u64 = 1_000;
const MASTER_RETRY_MAX_BACKOFF_MILLIS: u64 = 30_000;
const MASTER_LIFECYCLE_DECISION_MAX_ROUNDS: usize = 8;
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
            return Ok(ProductionMasterTickOutcome::Idle);
        }

        let mut latest_outcome = ProductionMasterTickOutcome::Idle;
        for event in inbox.events {
            let attempt = if state.retry_event_id.as_deref() == Some(event.event_id.as_str()) {
                state.retry_attempt
            } else {
                0
            };
            match self.handle_event(&task_runtime, &event, attempt) {
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
        Ok(latest_outcome)
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
    ) -> Result<Option<ProductionMasterTickOutcome>, ProductionMasterRunnerError> {
        let task = task_runtime
            .query_task(&event.task_id)
            .map_err(task_center_error)?;
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
    let lifecycle_session_id = format!("master-lifecycle-{event_key}-attempt-{attempt}");
    Ok(LiveReasonTurnRequest {
        runtime_home: runtime_home.to_path_buf(),
        session_id: SessionId::new(lifecycle_session_id),
        turn_id: TurnId::new(format!("master-lifecycle-{event_key}-decision")),
        trace_id: TraceId::new(format!("master-lifecycle-trace-{event_key}")),
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

fn sleep_with_cancel(cancel: &AtomicBool, duration: Duration) {
    let poll = Duration::from_millis(CANCEL_POLL_MILLIS);
    let mut remaining = duration;
    while !remaining.is_zero() && !cancel.load(Ordering::Acquire) {
        let step = remaining.min(poll);
        thread::sleep(step);
        remaining = remaining.saturating_sub(step);
    }
}
