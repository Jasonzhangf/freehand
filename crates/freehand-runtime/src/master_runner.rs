use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use freehand_config::{AgentMode, SelectedAgentConfig};
use freehand_contracts::{AgentId, SessionId, TerminalStatus, TraceId, TurnId};
use freehand_reason::{ReasonPersistence, ReasonPersistenceError};
use freehand_task::{
    TaskActor, TaskAppendRequest, TaskBoardQuery, TaskError, TaskEventInboxEntry,
    TaskEventInboxQuery, TaskId, TaskRuntime, TaskSnapshot, TaskStatus, TaskWatermark,
};
use freehand_ui_protocol::UiProtocolState;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use super::{
    DueTimerSchedule, LiveReasonExecutionProfile, LiveReasonTaskDecisionBoundary,
    LiveReasonTaskDecisionMode, LiveReasonTurnRequest, RuntimeAgentBootstrapError,
    apply_runtime_debug_event, apply_runtime_reason_broadcast, claim_due_timer_schedule,
    complete_due_timer_schedule, fail_due_timer_schedule, load_default_runtime_agent,
    now_unix_seconds, run_live_reason_turn, run_live_reason_turn_with_hooks,
    run_master_lifecycle_reason_turn, run_master_lifecycle_reason_turn_with_hooks,
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
const MASTER_ATTENTION_KIND_WEIGHT: i128 = 10_000;
const MASTER_ATTENTION_TASK_PRIORITY_WEIGHT: i128 = 100;
const MASTER_ATTENTION_ADMISSION_AGE_WEIGHT: i128 = 5_000;
const MASTER_ATTENTION_TASK_PRIORITY_MIN: i64 = -100;
const MASTER_ATTENTION_TASK_PRIORITY_MAX: i64 = 100;
pub(crate) const MASTER_ACTIVE_WORK_DEFAULT_PRIORITY: i64 = 90;
const MASTER_ACTIVE_WORK_SCORE_WEIGHT: i128 = MASTER_ATTENTION_KIND_WEIGHT;
static MASTER_ACTIVE_WORK_WRITE_COUNTER: AtomicU64 = AtomicU64::new(0);

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
    ParentEvaluationSkipped {
        parent_session_id: SessionId,
        evaluated_child_task_ids: Vec<TaskId>,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MasterActiveWorkState {
    Running,
    SuspendRequested,
    SuspendedByAttention,
    Restoring,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum MasterWorkSafePoint {
    BeforeProviderRequest,
    ProviderInFlight,
    BeforeToolExecution,
    ToolEffectInFlight,
    BeforeTerminalPersistence,
    TerminalPersistenceInFlight,
    BetweenRounds,
}

impl MasterWorkSafePoint {
    fn is_interruptible(self) -> bool {
        matches!(
            self,
            Self::BeforeProviderRequest
                | Self::BeforeToolExecution
                | Self::BeforeTerminalPersistence
                | Self::BetweenRounds
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MasterWorkReference {
    pub work_id: String,
    pub session_id: SessionId,
    pub logical_turn_id: TurnId,
    pub trace_id: TraceId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MasterAttentionReference {
    pub event_id: String,
    pub task_id: TaskId,
    pub kind: String,
    pub severity_rank: u8,
    pub task_priority: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MasterAttentionResolution {
    pub attention_event_id: String,
    pub decision_kind: String,
    pub changed_task_ids: Vec<TaskId>,
    pub changed_constraints: Vec<String>,
    pub resume_from: MasterWorkReference,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MasterActiveWorkCheckpoint {
    pub schema_version: u32,
    pub master_agent_id: AgentId,
    pub session_id: SessionId,
    pub logical_turn_id: TurnId,
    pub trace_id: TraceId,
    pub work_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_process_id: Option<u32>,
    pub priority: i64,
    pub state: MasterActiveWorkState,
    pub safe_point: MasterWorkSafePoint,
    pub parent_objective_reference: String,
    pub active_task_or_event_cursor: Option<String>,
    pub permitted_resume_context: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub suspend_requested_by: Option<MasterAttentionReference>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub attention_resolution: Option<MasterAttentionResolution>,
    pub updated_at: u64,
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

struct LiveMasterTurnExecutor {
    ui_state: Option<Arc<Mutex<UiProtocolState>>>,
}

impl LiveMasterTurnExecutor {
    fn new(ui_state: Option<Arc<Mutex<UiProtocolState>>>) -> Self {
        Self { ui_state }
    }

    fn execute_reason_turn(
        &self,
        selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
        decision_boundary: Option<LiveReasonTaskDecisionBoundary>,
    ) -> Result<super::LiveReasonTurnOutcome, super::RuntimeLiveBridgeError> {
        let Some(ui_state) = self.ui_state.as_ref().map(Arc::clone) else {
            return match decision_boundary {
                Some(boundary) => run_master_lifecycle_reason_turn(selected, request, boundary),
                None => run_live_reason_turn(selected, request),
            };
        };
        let reason_agent_id = AgentId::new(selected.name.clone());
        let master_node_id = selected.node_id.clone();
        match decision_boundary {
            Some(boundary) => run_master_lifecycle_reason_turn_with_hooks(
                selected,
                request,
                boundary,
                |event| {
                    apply_runtime_reason_broadcast(
                        &ui_state,
                        &reason_agent_id,
                        &master_node_id,
                        event,
                    );
                },
                |event| {
                    apply_runtime_debug_event(&ui_state, &reason_agent_id, &master_node_id, event);
                },
                |projection| {
                    ui_state
                        .lock()
                        .expect("lock ui state")
                        .publish_task_list_projection(projection.clone());
                },
            ),
            None => run_live_reason_turn_with_hooks(
                selected,
                request,
                |event| {
                    apply_runtime_reason_broadcast(
                        &ui_state,
                        &reason_agent_id,
                        &master_node_id,
                        event,
                    );
                },
                |event| {
                    apply_runtime_debug_event(&ui_state, &reason_agent_id, &master_node_id, event);
                },
                |projection| {
                    ui_state
                        .lock()
                        .expect("lock ui state")
                        .publish_task_list_projection(projection.clone());
                },
            ),
        }
    }
}

impl MasterTurnExecutor for LiveMasterTurnExecutor {
    fn execute(
        &self,
        selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
        decision_boundary: LiveReasonTaskDecisionBoundary,
    ) -> Result<String, String> {
        let outcome = self
            .execute_reason_turn(selected, request, Some(decision_boundary))
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
        let outcome = self
            .execute_reason_turn(selected, request, None)
            .map_err(|error| error.to_string())?;
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
        let outcome = self
            .execute_reason_turn(selected, request, None)
            .map_err(|error| error.to_string())?;
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
    #[serde(default)]
    skipped_parent_evaluations: BTreeSet<String>,
    #[serde(default)]
    pending_attention: Vec<MasterAttentionItem>,
    #[serde(default)]
    next_attention_sequence: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MasterAttentionItem {
    event: TaskEventInboxEntry,
    severity_rank: u8,
    task_priority: i64,
    admitted_sequence: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MasterBusyAttentionDecision {
    Proceed,
    Deferred,
    SuspendRequested,
    Suspended,
}

pub(crate) fn register_master_active_work(
    runtime_home: &Path,
    master_agent_id: &AgentId,
    session_id: &SessionId,
    logical_turn_id: &TurnId,
    trace_id: &TraceId,
) -> Result<(), String> {
    with_master_active_work_lock(runtime_home, master_agent_id, || {
        if let Some(existing) = load_master_active_work_unlocked(runtime_home, master_agent_id)? {
            if existing.logical_turn_id == *logical_turn_id
                && existing.session_id == *session_id
                && existing.trace_id == *trace_id
            {
                return Ok(());
            }
            return Err(format!(
                "Master active work `{}` is still open; cannot register concurrent turn `{}`",
                existing.work_id,
                logical_turn_id.as_str()
            ));
        }
        let work_id = format!(
            "{}:{}:{}",
            master_agent_id.as_str(),
            session_id.as_str(),
            logical_turn_id.as_str()
        );
        let checkpoint = MasterActiveWorkCheckpoint {
            schema_version: 1,
            master_agent_id: master_agent_id.clone(),
            session_id: session_id.clone(),
            logical_turn_id: logical_turn_id.clone(),
            trace_id: trace_id.clone(),
            work_id,
            owner_process_id: Some(std::process::id()),
            priority: MASTER_ACTIVE_WORK_DEFAULT_PRIORITY,
            state: MasterActiveWorkState::Running,
            safe_point: MasterWorkSafePoint::BeforeProviderRequest,
            parent_objective_reference: format!(
                "reason://{}/{}",
                session_id.as_str(),
                logical_turn_id.as_str()
            ),
            active_task_or_event_cursor: None,
            permitted_resume_context: vec![
                "attention_event_id".to_owned(),
                "changed_task_ids".to_owned(),
                "decision_kind".to_owned(),
                "changed_constraints".to_owned(),
                "resume_from".to_owned(),
            ],
            suspend_requested_by: None,
            attention_resolution: None,
            updated_at: now_unix_seconds(),
        };
        write_master_active_work_unlocked(runtime_home, &checkpoint)
    })
}

pub(crate) fn clear_master_active_work_if_current(
    runtime_home: &Path,
    master_agent_id: &AgentId,
    logical_turn_id: &TurnId,
) -> Result<(), String> {
    with_master_active_work_lock(runtime_home, master_agent_id, || {
        let Some(checkpoint) = load_master_active_work_unlocked(runtime_home, master_agent_id)?
        else {
            return Ok(());
        };
        if checkpoint.logical_turn_id != *logical_turn_id {
            return Ok(());
        }
        let path = master_active_work_path(runtime_home, master_agent_id);
        match fs::remove_file(path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.to_string()),
        }
    })
}

fn master_active_work_path(runtime_home: &Path, master_agent_id: &AgentId) -> PathBuf {
    runtime_home
        .join("state")
        .join("master-loop")
        .join(format!("{}.active-work.json", master_agent_id.as_str()))
}

fn master_active_work_lock_path(runtime_home: &Path, master_agent_id: &AgentId) -> PathBuf {
    runtime_home
        .join("state")
        .join("master-loop")
        .join(format!("{}.active-work.lock", master_agent_id.as_str()))
}

fn with_master_active_work_lock<T>(
    runtime_home: &Path,
    master_agent_id: &AgentId,
    action: impl FnOnce() -> Result<T, String>,
) -> Result<T, String> {
    let lock_path = master_active_work_lock_path(runtime_home, master_agent_id);
    let parent = lock_path
        .parent()
        .ok_or_else(|| "active Master work lock path has no parent".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let lock_file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .map_err(|error| error.to_string())?;
    FileExt::lock_exclusive(&lock_file).map_err(|error| error.to_string())?;
    let result = action();
    let unlock_result = FileExt::unlock(&lock_file).map_err(|error| error.to_string());
    match (result, unlock_result) {
        (Ok(value), Ok(())) => Ok(value),
        (Err(error), Ok(())) => Err(error),
        (Ok(_), Err(error)) => Err(error),
        (Err(error), Err(unlock_error)) => Err(format!(
            "{error}; additionally failed to unlock active Master work: {unlock_error}"
        )),
    }
}

pub(crate) fn load_master_active_work(
    runtime_home: &Path,
    master_agent_id: &AgentId,
) -> Result<Option<MasterActiveWorkCheckpoint>, String> {
    with_master_active_work_lock(runtime_home, master_agent_id, || {
        load_master_active_work_unlocked(runtime_home, master_agent_id)
    })
}

fn load_master_active_work_unlocked(
    runtime_home: &Path,
    master_agent_id: &AgentId,
) -> Result<Option<MasterActiveWorkCheckpoint>, String> {
    let path = master_active_work_path(runtime_home, master_agent_id);
    if !path.exists() {
        return Ok(None);
    }
    let raw = fs::read_to_string(&path).map_err(|error| error.to_string())?;
    let checkpoint: MasterActiveWorkCheckpoint =
        serde_json::from_str(&raw).map_err(|error| error.to_string())?;
    if checkpoint.master_agent_id != *master_agent_id {
        return Err(format!(
            "active Master work owner mismatch: expected `{}`, got `{}`",
            master_agent_id.as_str(),
            checkpoint.master_agent_id.as_str()
        ));
    }
    Ok(Some(checkpoint))
}

pub(crate) fn recoverable_stale_master_active_work(
    runtime_home: &Path,
    master_agent_id: &AgentId,
    task_runtime: &TaskRuntime,
) -> Result<Option<MasterActiveWorkCheckpoint>, String> {
    let Some(checkpoint) = load_master_active_work(runtime_home, master_agent_id)? else {
        return Ok(None);
    };
    if let Some(owner_process_id) = checkpoint.owner_process_id {
        if owner_process_id == std::process::id() || process_is_alive(owner_process_id) {
            return Ok(None);
        }
        return Ok(Some(checkpoint));
    }
    if matches!(
        checkpoint.state,
        MasterActiveWorkState::SuspendedByAttention | MasterActiveWorkState::Restoring
    ) && suspended_attention_has_owner_decision(&checkpoint, master_agent_id, task_runtime)?
    {
        return Ok(Some(checkpoint));
    }
    Ok(None)
}

fn suspended_attention_has_owner_decision(
    checkpoint: &MasterActiveWorkCheckpoint,
    master_agent_id: &AgentId,
    task_runtime: &TaskRuntime,
) -> Result<bool, String> {
    let Some(reference) = checkpoint.suspend_requested_by.as_ref() else {
        return Ok(false);
    };
    let history = match task_runtime.task_history(&reference.task_id) {
        Ok(history) => history,
        Err(TaskError::TaskNotFound(_)) => return Ok(false),
        Err(error) => return Err(error.to_string()),
    };
    let Some(reference_index) = history
        .iter()
        .position(|event| event.event_id == reference.event_id)
    else {
        return Ok(false);
    };
    Ok(history.iter().skip(reference_index + 1).any(|event| {
        event.actor.agent_id == *master_agent_id
            && matches!(
                event.event_type.as_str(),
                "TaskProgressed"
                    | "TaskAssigned"
                    | "TaskReviewApproved"
                    | "TaskReviewRejected"
                    | "TaskClosed"
                    | "TaskCancelled"
            )
    }))
}

fn process_is_alive(process_id: u32) -> bool {
    if process_id == 0 {
        return false;
    }
    #[cfg(unix)]
    {
        let result = unsafe { libc::kill(process_id as libc::pid_t, 0) };
        if result == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
    #[cfg(not(unix))]
    {
        false
    }
}

fn write_master_active_work_unlocked(
    runtime_home: &Path,
    checkpoint: &MasterActiveWorkCheckpoint,
) -> Result<(), String> {
    let path = master_active_work_path(runtime_home, &checkpoint.master_agent_id);
    let parent = path
        .parent()
        .ok_or_else(|| "active Master work path has no parent".to_owned())?;
    fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    let unique = MASTER_ACTIVE_WORK_WRITE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let temp = path.with_extension(format!("active-work.tmp-{}-{unique}", std::process::id()));
    let raw = serde_json::to_string_pretty(checkpoint).map_err(|error| error.to_string())?;
    fs::write(&temp, raw).map_err(|error| error.to_string())?;
    if let Err(rename_error) = fs::rename(&temp, &path) {
        let cleanup_result = fs::remove_file(&temp);
        return match cleanup_result {
            Ok(()) => Err(rename_error.to_string()),
            Err(cleanup_error) => Err(format!(
                "{}; additionally failed to remove temp active-work file `{}`: {}",
                rename_error,
                temp.display(),
                cleanup_error
            )),
        };
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn update_master_active_work_safe_point(
    runtime_home: &Path,
    master_agent_id: &AgentId,
    safe_point: MasterWorkSafePoint,
) -> Result<MasterActiveWorkCheckpoint, String> {
    with_master_active_work_lock(runtime_home, master_agent_id, || {
        let mut checkpoint = load_master_active_work_unlocked(runtime_home, master_agent_id)?
            .ok_or_else(|| "active Master work is missing".to_owned())?;
        checkpoint.safe_point = safe_point;
        checkpoint.updated_at = now_unix_seconds();
        write_master_active_work_unlocked(runtime_home, &checkpoint)?;
        Ok(checkpoint)
    })
}

pub(crate) fn record_master_active_work_safe_point_if_current(
    runtime_home: &Path,
    master_agent_id: &AgentId,
    session_id: &SessionId,
    logical_turn_id: &TurnId,
    safe_point: MasterWorkSafePoint,
) -> Result<Option<MasterActiveWorkCheckpoint>, String> {
    with_master_active_work_lock(runtime_home, master_agent_id, || {
        let Some(mut checkpoint) = load_master_active_work_unlocked(runtime_home, master_agent_id)?
        else {
            return Ok(None);
        };
        if checkpoint.session_id != *session_id || checkpoint.logical_turn_id != *logical_turn_id {
            return Ok(None);
        }
        checkpoint.safe_point = safe_point;
        if checkpoint.state == MasterActiveWorkState::SuspendRequested
            && checkpoint.safe_point.is_interruptible()
        {
            checkpoint.state = MasterActiveWorkState::SuspendedByAttention;
        }
        checkpoint.updated_at = now_unix_seconds();
        write_master_active_work_unlocked(runtime_home, &checkpoint)?;
        Ok(Some(checkpoint))
    })
}

pub(crate) fn inspect_master_active_work_if_current(
    runtime_home: &Path,
    master_agent_id: &AgentId,
    session_id: &SessionId,
    logical_turn_id: &TurnId,
    trace_id: &TraceId,
) -> Result<Option<MasterActiveWorkCheckpoint>, String> {
    with_master_active_work_lock(runtime_home, master_agent_id, || {
        let Some(checkpoint) = load_master_active_work_unlocked(runtime_home, master_agent_id)?
        else {
            return Ok(None);
        };
        if checkpoint.session_id != *session_id
            || checkpoint.logical_turn_id != *logical_turn_id
            || checkpoint.trace_id != *trace_id
        {
            return Err(format!(
                "active Master work `{}` does not match live turn `{}/{}/{}`",
                checkpoint.work_id,
                session_id.as_str(),
                logical_turn_id.as_str(),
                trace_id.as_str()
            ));
        }
        Ok(Some(checkpoint))
    })
}

pub(crate) fn take_master_attention_resolution_if_current(
    runtime_home: &Path,
    master_agent_id: &AgentId,
    session_id: &SessionId,
    logical_turn_id: &TurnId,
    trace_id: &TraceId,
) -> Result<Option<MasterAttentionResolution>, String> {
    with_master_active_work_lock(runtime_home, master_agent_id, || {
        let Some(mut checkpoint) = load_master_active_work_unlocked(runtime_home, master_agent_id)?
        else {
            return Ok(None);
        };
        if checkpoint.session_id != *session_id
            || checkpoint.logical_turn_id != *logical_turn_id
            || checkpoint.trace_id != *trace_id
        {
            return Err(format!(
                "active Master work `{}` does not match live turn `{}/{}/{}`",
                checkpoint.work_id,
                session_id.as_str(),
                logical_turn_id.as_str(),
                trace_id.as_str()
            ));
        }
        if checkpoint.state != MasterActiveWorkState::Running {
            return Ok(None);
        }
        let Some(resolution) = checkpoint.attention_resolution.take() else {
            return Ok(None);
        };
        validate_master_attention_resolution(&resolution)?;
        if resolution.resume_from.work_id != checkpoint.work_id
            || resolution.resume_from.session_id != checkpoint.session_id
            || resolution.resume_from.logical_turn_id != checkpoint.logical_turn_id
            || resolution.resume_from.trace_id != checkpoint.trace_id
        {
            return Err(format!(
                "Master attention resolution return identity mismatch for active work `{}`",
                checkpoint.work_id
            ));
        }
        checkpoint.updated_at = now_unix_seconds();
        write_master_active_work_unlocked(runtime_home, &checkpoint)?;
        Ok(Some(resolution))
    })
}

fn validate_master_attention_resolution(
    resolution: &MasterAttentionResolution,
) -> Result<(), String> {
    const FORBIDDEN_FIELDS: [&str; 4] = [
        "raw_worker_transcript",
        "raw_control_turn_transcript",
        "provider_request_payload",
        "provider_response_payload",
    ];
    if resolution
        .changed_constraints
        .iter()
        .any(|value| FORBIDDEN_FIELDS.iter().any(|field| value.contains(field)))
    {
        return Err(
            "Master attention resolution contains forbidden raw transcript/provider payload"
                .to_owned(),
        );
    }
    Ok(())
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
            Arc::new(LiveMasterTurnExecutor::new(None)),
        )
    }

    pub fn from_selected_agent_with_ui_state(
        selected: SelectedAgentConfig,
        runtime_home: PathBuf,
        ui_state: Arc<Mutex<UiProtocolState>>,
    ) -> Result<Self, ProductionMasterRunnerError> {
        Self::from_selected_agent_with_executor(
            selected,
            runtime_home,
            Arc::new(LiveMasterTurnExecutor::new(Some(ui_state))),
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

        let inbox = self.query_event_inbox_repairing_stale_cursor(&task_runtime, &mut state)?;
        self.admit_attention_events(&task_runtime, &mut state, inbox.events)?;

        loop {
            if state.pending_attention.is_empty() {
                if let Some(outcome) =
                    self.reconcile_blocked_parent_worksets(&task_runtime, &mut state)?
                {
                    self.write_state(&state)?;
                    return Ok(outcome);
                }
                if let Some(outcome) =
                    self.reconcile_closed_parent_worksets(&task_runtime, &mut state)?
                {
                    self.write_state(&state)?;
                    return Ok(outcome);
                }
                self.write_state(&state)?;
                if let Some(outcome) = self.handle_due_timer()? {
                    return Ok(outcome);
                }
                return Ok(ProductionMasterTickOutcome::Idle);
            }

            let attention_index = highest_priority_attention_index(
                &state.pending_attention,
                state.next_attention_sequence,
            )
            .expect("pending attention is non-empty");
            let attention = state.pending_attention[attention_index].clone();
            let busy_decision = self.apply_busy_attention_policy(&attention)?;
            if matches!(
                busy_decision,
                MasterBusyAttentionDecision::Deferred
                    | MasterBusyAttentionDecision::SuspendRequested
            ) {
                self.write_state(&state)?;
                return Ok(ProductionMasterTickOutcome::Idle);
            }
            let event = attention.event.clone();
            let attempt = if state.retry_event_id.as_deref() == Some(event.event_id.as_str()) {
                state.retry_attempt
            } else {
                0
            };
            let outcome = match self.handle_event(&task_runtime, &event, attempt, &mut state) {
                Ok(outcome) => outcome,
                Err(error) => {
                    if error.is_retryable_lifecycle_failure() {
                        state.retry_event_id = Some(event.event_id.clone());
                        state.retry_attempt = attempt.saturating_add(1);
                        self.write_state(&state)?;
                    }
                    return Err(error);
                }
            };
            if matches!(busy_decision, MasterBusyAttentionDecision::Suspended) {
                self.restore_active_work_after_attention(&attention, outcome.as_ref())?;
            }
            state.pending_attention.remove(attention_index);
            state.retry_event_id = None;
            state.retry_attempt = 0;
            self.write_state(&state)?;
            if let Some(outcome) = outcome {
                return Ok(outcome);
            }
        }
    }

    fn admit_attention_events(
        &self,
        task_runtime: &TaskRuntime,
        state: &mut MasterLoopState,
        events: Vec<TaskEventInboxEntry>,
    ) -> Result<(), ProductionMasterRunnerError> {
        let mut admitted_any = false;
        for event in events {
            state.cursor = Some(event.cursor.clone());
            if !master_event_requires_attention(&event.kind)
                || state
                    .pending_attention
                    .iter()
                    .any(|item| item.event.event_id == event.event_id)
            {
                continue;
            }
            let task = match task_runtime.query_task(&event.task_id) {
                Ok(task) => task,
                Err(TaskError::TaskNotFound(task_id)) => {
                    eprintln!(
                        "master lifecycle runner skipped stale EventInbox event `{}` for missing task `{task_id}`",
                        event.event_id
                    );
                    continue;
                }
                Err(error) => return Err(task_center_error(error)),
            };
            let admitted_sequence = state.next_attention_sequence;
            state.next_attention_sequence = state.next_attention_sequence.saturating_add(1);
            state.pending_attention.push(MasterAttentionItem {
                severity_rank: master_attention_severity_rank(&event),
                task_priority: task.priority,
                event,
                admitted_sequence,
            });
            admitted_any = true;
        }
        if admitted_any || state.cursor.is_some() {
            self.write_state(state)?;
        }
        Ok(())
    }

    fn apply_busy_attention_policy(
        &self,
        attention: &MasterAttentionItem,
    ) -> Result<MasterBusyAttentionDecision, ProductionMasterRunnerError> {
        with_master_active_work_lock(&self.runtime_home, &self.master_agent_id, || {
            let Some(mut active_work) =
                load_master_active_work_unlocked(&self.runtime_home, &self.master_agent_id)?
            else {
                return Ok(MasterBusyAttentionDecision::Proceed);
            };
            let attention_reference = MasterAttentionReference {
                event_id: attention.event.event_id.clone(),
                task_id: attention.event.task_id.clone(),
                kind: attention.event.kind.clone(),
                severity_rank: attention.severity_rank,
                task_priority: attention.task_priority,
            };
            if active_work.state == MasterActiveWorkState::SuspendedByAttention
                && active_work.suspend_requested_by.as_ref() == Some(&attention_reference)
            {
                return Ok(MasterBusyAttentionDecision::Suspended);
            }
            let attention_score = master_attention_preemption_score(attention);
            let active_score =
                i128::from(active_work.priority.clamp(0, 100)) * MASTER_ACTIVE_WORK_SCORE_WEIGHT;
            if attention_score <= active_score {
                return Ok(MasterBusyAttentionDecision::Deferred);
            }
            active_work.suspend_requested_by = Some(attention_reference);
            active_work.attention_resolution = None;
            active_work.updated_at = now_unix_seconds();
            if !active_work.safe_point.is_interruptible() {
                active_work.state = MasterActiveWorkState::SuspendRequested;
                write_master_active_work_unlocked(&self.runtime_home, &active_work)?;
                return Ok(MasterBusyAttentionDecision::SuspendRequested);
            }
            active_work.state = MasterActiveWorkState::SuspendedByAttention;
            write_master_active_work_unlocked(&self.runtime_home, &active_work)?;
            Ok(MasterBusyAttentionDecision::Suspended)
        })
        .map_err(ProductionMasterRunnerError::State)
    }

    fn restore_active_work_after_attention(
        &self,
        attention: &MasterAttentionItem,
        outcome: Option<&ProductionMasterTickOutcome>,
    ) -> Result<(), ProductionMasterRunnerError> {
        with_master_active_work_lock(&self.runtime_home, &self.master_agent_id, || {
            let Some(mut active_work) =
                load_master_active_work_unlocked(&self.runtime_home, &self.master_agent_id)?
            else {
                return Err(
                    "cannot restore Master work without an active-work checkpoint".to_owned(),
                );
            };
            let reference = active_work.suspend_requested_by.as_ref().ok_or_else(|| {
                "cannot restore Master work without suspended attention identity".to_owned()
            })?;
            if reference.event_id != attention.event.event_id {
                return Err(format!(
                    "active Master work is suspended by `{}`, not `{}`",
                    reference.event_id, attention.event.event_id
                ));
            }
            let (decision_kind, changed_task_ids) = match outcome {
                Some(ProductionMasterTickOutcome::TaskAdvanced { task_id, .. }) => {
                    ("task_advanced".to_owned(), vec![task_id.clone()])
                }
                Some(ProductionMasterTickOutcome::BlockedObserved { task_id, .. }) => (
                    "blocked_decision_recorded".to_owned(),
                    vec![task_id.clone()],
                ),
                Some(ProductionMasterTickOutcome::ParentEvaluated {
                    evaluated_child_task_ids,
                    ..
                }) => (
                    "parent_goal_evaluated".to_owned(),
                    evaluated_child_task_ids.clone(),
                ),
                Some(ProductionMasterTickOutcome::ParentEvaluationSkipped {
                    evaluated_child_task_ids,
                    ..
                }) => (
                    "parent_goal_evaluation_skipped".to_owned(),
                    evaluated_child_task_ids.clone(),
                ),
                Some(ProductionMasterTickOutcome::TimerFired { .. }) => {
                    return Err("timer outcome cannot resolve Task Center attention".to_owned());
                }
                Some(ProductionMasterTickOutcome::Idle) | None => {
                    ("attention_noop".to_owned(), Vec::new())
                }
            };
            let resolution = MasterAttentionResolution {
                attention_event_id: attention.event.event_id.clone(),
                decision_kind,
                changed_task_ids,
                changed_constraints: Vec::new(),
                resume_from: MasterWorkReference {
                    work_id: active_work.work_id.clone(),
                    session_id: active_work.session_id.clone(),
                    logical_turn_id: active_work.logical_turn_id.clone(),
                    trace_id: active_work.trace_id.clone(),
                },
            };
            validate_master_attention_resolution(&resolution)?;
            active_work.state = MasterActiveWorkState::Restoring;
            active_work.attention_resolution = Some(resolution);
            active_work.updated_at = now_unix_seconds();
            write_master_active_work_unlocked(&self.runtime_home, &active_work)?;
            active_work.state = MasterActiveWorkState::Running;
            active_work.safe_point = MasterWorkSafePoint::BetweenRounds;
            active_work.suspend_requested_by = None;
            active_work.updated_at = now_unix_seconds();
            write_master_active_work_unlocked(&self.runtime_home, &active_work)
        })
        .map_err(ProductionMasterRunnerError::State)
    }

    fn query_event_inbox_repairing_stale_cursor(
        &self,
        task_runtime: &TaskRuntime,
        state: &mut MasterLoopState,
    ) -> Result<freehand_task::TaskEventInboxProjection, ProductionMasterRunnerError> {
        match task_runtime.query_event_inbox(TaskEventInboxQuery {
            after_cursor: state.cursor.clone(),
            limit: usize::MAX,
        }) {
            Ok(inbox) => Ok(inbox),
            Err(TaskError::CursorNotFound(cursor)) => {
                eprintln!(
                    "master lifecycle runner ignored stale EventInbox cursor `{cursor}` and replayed current Task Center ledger truth"
                );
                state.cursor = None;
                self.write_state(state)?;
                task_runtime
                    .query_event_inbox(TaskEventInboxQuery {
                        after_cursor: None,
                        limit: usize::MAX,
                    })
                    .map_err(task_center_error)
            }
            Err(error) => Err(task_center_error(error)),
        }
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
            timer_live_request(&self.runtime_home, &self.master_agent_id, &due)?,
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
        let task = match task_runtime.query_task(&event.task_id) {
            Ok(task) => task,
            Err(TaskError::TaskNotFound(task_id)) => {
                eprintln!(
                    "master lifecycle runner dropped stale pending attention `{}` for missing task `{task_id}`",
                    event.event_id
                );
                return Ok(None);
            }
            Err(error) => return Err(task_center_error(error)),
        };
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
            "execution_attention_required" => task.status == TaskStatus::Interrupted,
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
        let worker_names = self.selected.worker_peer_names().join(", ");
        let agent_board_json = serde_json::to_string_pretty(
            &task_runtime
                .query_agent_board()
                .map_err(task_center_error)?,
        )
        .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
        let summary = self
            .executor
            .execute(
                &self.selected,
                master_live_request(
                    &self.runtime_home,
                    &worker_names,
                    &agent_board_json,
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
        let parent_turn_id = closed_task.parent.turn_id.clone();
        let children =
            parent_workset_children(task_runtime, &parent_session_id, parent_turn_id.as_ref())?;
        self.evaluate_closed_parent_workset(
            task_runtime,
            parent_session_id,
            children,
            attempt,
            state,
        )
    }

    fn reconcile_closed_parent_worksets(
        &self,
        task_runtime: &TaskRuntime,
        state: &mut MasterLoopState,
    ) -> Result<Option<ProductionMasterTickOutcome>, ProductionMasterRunnerError> {
        let board = task_runtime
            .query_task_board(TaskBoardQuery {
                status: None,
                assignee: None,
                include_terminal: true,
            })
            .map_err(task_center_error)?;
        let mut groups = BTreeMap::<String, Vec<TaskSnapshot>>::new();
        for task in board.tasks {
            let Some(parent_session_id) = task.parent.session_id.as_ref() else {
                continue;
            };
            let parent_turn_id = parent_turn_group_key(task.parent.turn_id.as_ref());
            groups
                .entry(format!("{}|{}", parent_session_id.as_str(), parent_turn_id))
                .or_default()
                .push(task);
        }
        for mut children in groups.into_values() {
            children.sort_by(|left, right| left.task_id.cmp(&right.task_id));
            if children.is_empty()
                || children
                    .iter()
                    .any(|task| task.status != TaskStatus::Closed)
            {
                continue;
            }
            let Some(parent_session_id) = children[0].parent.session_id.clone() else {
                continue;
            };
            let parent_turn_id = children[0].parent.turn_id.clone();
            if !parent_logical_turn_waits_for_lifecycle(
                &self.runtime_home,
                &self.master_agent_id,
                &parent_session_id,
                parent_turn_id.as_ref(),
            )? {
                continue;
            }
            if let Some(outcome) = self.evaluate_closed_parent_workset(
                task_runtime,
                parent_session_id,
                children,
                0,
                state,
            )? {
                return Ok(Some(outcome));
            }
        }
        Ok(None)
    }

    fn reconcile_blocked_parent_worksets(
        &self,
        task_runtime: &TaskRuntime,
        state: &mut MasterLoopState,
    ) -> Result<Option<ProductionMasterTickOutcome>, ProductionMasterRunnerError> {
        let board = task_runtime
            .query_task_board(TaskBoardQuery {
                status: None,
                assignee: None,
                include_terminal: true,
            })
            .map_err(task_center_error)?;
        let mut groups = BTreeMap::<String, Vec<TaskSnapshot>>::new();
        for task in board.tasks {
            let Some(parent_session_id) = task.parent.session_id.as_ref() else {
                continue;
            };
            let parent_turn_id = parent_turn_group_key(task.parent.turn_id.as_ref());
            groups
                .entry(format!("{}|{}", parent_session_id.as_str(), parent_turn_id))
                .or_default()
                .push(task);
        }
        for mut children in groups.into_values() {
            children.sort_by(|left, right| left.task_id.cmp(&right.task_id));
            let Some(parent_session_id) = children
                .first()
                .and_then(|task| task.parent.session_id.clone())
            else {
                continue;
            };
            let parent_turn_id = children[0].parent.turn_id.clone();
            if !parent_logical_turn_waits_for_lifecycle(
                &self.runtime_home,
                &self.master_agent_id,
                &parent_session_id,
                parent_turn_id.as_ref(),
            )? {
                continue;
            }
            if children.iter().any(|task| {
                !matches!(
                    task.status,
                    TaskStatus::Blocked
                        | TaskStatus::Closed
                        | TaskStatus::Cancelled
                        | TaskStatus::Failed
                )
            }) {
                continue;
            }
            let mut blocked = Vec::new();
            for task in &children {
                if let Some(truth) =
                    parent_blocked_subtask_truth(task_runtime, &self.master_agent_id, task)?
                {
                    blocked.push(truth);
                }
            }
            if blocked.is_empty() {
                continue;
            }
            let decision_key = blocked
                .iter()
                .map(|task| format!("{}:{}", task.task_id.as_str(), task.decision_seq))
                .collect::<Vec<_>>()
                .join(",");
            let evaluation_key = format!(
                "blocked|{}|{}|{}",
                parent_session_id.as_str(),
                parent_turn_group_key(parent_turn_id.as_ref()),
                decision_key
            );
            if state.completed_parent_evaluations.contains(&evaluation_key) {
                continue;
            }
            let evaluation_marker = parent_evaluation_marker(&evaluation_key);
            if persisted_parent_blocked_follow_up_summary(
                &self.runtime_home,
                &self.master_agent_id,
                &parent_session_id,
                &evaluation_marker,
            )?
            .is_some()
            {
                state.completed_parent_evaluations.insert(evaluation_key);
                continue;
            }
            let Some(user_objectives) = parent_user_objectives(
                &self.runtime_home,
                &self.master_agent_id,
                &parent_session_id,
            )?
            else {
                continue;
            };
            let evaluation_turn_id = next_parent_evaluation_turn_id(
                &self.runtime_home,
                &self.master_agent_id,
                &parent_session_id,
            )?;
            let request = parent_blocked_follow_up_live_request(
                &self.runtime_home,
                &parent_session_id,
                &evaluation_turn_id,
                &evaluation_marker,
                &user_objectives,
                &blocked,
            )?;
            let summary = self
                .executor
                .execute_parent_evaluation(&self.selected, request)
                .map_err(ProductionMasterRunnerError::Execution)?;
            state.completed_parent_evaluations.insert(evaluation_key);
            return Ok(Some(ProductionMasterTickOutcome::ParentEvaluated {
                parent_session_id,
                evaluated_child_task_ids: blocked.into_iter().map(|task| task.task_id).collect(),
                summary,
            }));
        }
        Ok(None)
    }

    fn evaluate_closed_parent_workset(
        &self,
        task_runtime: &TaskRuntime,
        parent_session_id: SessionId,
        mut children: Vec<TaskSnapshot>,
        attempt: u32,
        state: &mut MasterLoopState,
    ) -> Result<Option<ProductionMasterTickOutcome>, ProductionMasterRunnerError> {
        if children.is_empty()
            || children
                .iter()
                .any(|task| task.status != TaskStatus::Closed)
        {
            return Ok(None);
        }
        children.sort_by(|left, right| left.task_id.cmp(&right.task_id));
        let evaluation_key = parent_evaluation_key(&parent_session_id, &children);
        if state.completed_parent_evaluations.contains(&evaluation_key)
            || state.skipped_parent_evaluations.contains(&evaluation_key)
        {
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
        let Some(user_objectives) = parent_user_objectives(
            &self.runtime_home,
            &self.master_agent_id,
            &parent_session_id,
        )?
        else {
            let reason = format!(
                "parent session `{}` has no persisted user objective truth; skipping this closed-child-set evaluation without finalizing",
                parent_session_id.as_str()
            );
            eprintln!("master lifecycle runner skipped parent evaluation: {reason}");
            state.skipped_parent_evaluations.insert(evaluation_key);
            return Ok(Some(ProductionMasterTickOutcome::ParentEvaluationSkipped {
                parent_session_id,
                evaluated_child_task_ids: children.into_iter().map(|task| task.task_id).collect(),
                reason,
            }));
        };
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
    worker_names: &str,
    agent_board_json: &str,
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
Configured Worker ids: {worker_names}\n\
\n\
Rules:\n\
- review_ready: query/history, then reject with concrete requirements or approve and close. Approved is not terminal; close it in the same lifecycle decision.\n\
- execution_blocked: inspect the blocker. Reassign only when retry is justified; otherwise call task(op=\"append\", task_id=<task-id>, note=\"blocked_decision: <required external action>\") to persist why it remains blocked.\n\
- execution_interrupted: treat the task as schedulable work, not as session-owned Worker failure. Use TaskHistory plus AgentBoard to choose retry_same_worker or takeover_to_another_available_configured_worker. Reassign the same task_id; do not create a duplicate task for the same objective.\n\
- execution_attention_required: inspect severity, change_kind, reason, evidence, and proposed_adjustment. Compare the changed Worker report against the original task goal, deliverables, acceptance, and current parent objective. Persist the adjustment on the same task with task(op=\"append\") and/or reassign the same task_id after changing requirements; do not mark success, do not create a duplicate task, and do not bury it as a generic blocker.\n\
- one trigger event owns one decision turn. After the required Task Center mutation is persisted, stop; never wait inside this turn for a future Worker event.\n\
- never fabricate completion, approval, evidence, or task state.\n\
\n\
Resource model:\n\
- Agent is a reusable execution resource in the pool, independent of session ownership.\n\
- Session is the user goal/progress context.\n\
- Task is the schedulable work item attached to a parent session when present.\n\
- Assignment, lease, and execution are temporary bindings between task and agent.\n\
- Retrying or taking over must preserve task_id and parent_session_id while creating a new execution later.\n\
\n\
AgentBoard resource truth:\n\
{agent_board_json}\n\
\n\
Task snapshot:\n{task_json}\n\
\n\
Trigger event:\n{event_json}"
        ),
        attachments: Vec::new(),
        attachment_metadata: Vec::new(),
        cwd: Some(runtime_home.to_path_buf()),
        execution_profile: LiveReasonExecutionProfile::Workspace,
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

#[derive(Debug, Serialize)]
struct ParentBlockedSubtaskTruth {
    task_id: TaskId,
    title: String,
    goal: String,
    status: String,
    blocked_reason: String,
    blocked_evidence: Vec<String>,
    master_decision: String,
    decision_seq: u64,
}

fn parent_blocked_subtask_truth(
    task_runtime: &TaskRuntime,
    master_agent_id: &AgentId,
    task: &TaskSnapshot,
) -> Result<Option<ParentBlockedSubtaskTruth>, ProductionMasterRunnerError> {
    if task.status != TaskStatus::Blocked {
        return Ok(None);
    }
    let history = task_runtime
        .task_history(&task.task_id)
        .map_err(task_center_error)?;
    let Some(blocked_index) = history
        .iter()
        .rposition(|event| event.event_type == "TaskBlocked")
    else {
        return Ok(None);
    };
    let decision = history
        .iter()
        .skip(blocked_index.saturating_add(1))
        .rev()
        .find(|event| {
            event.event_type == "TaskProgressed"
                && &event.actor.agent_id == master_agent_id
                && event
                    .payload
                    .get("note")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|note| note.starts_with("blocked_decision:"))
        });
    let Some(decision) = decision else {
        return Ok(None);
    };
    let blocked = &history[blocked_index];
    let blocked_reason = blocked
        .payload
        .get("reason")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Worker reported a blocked execution")
        .to_owned();
    let blocked_evidence = blocked
        .payload
        .get("evidence")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::to_owned)
        .collect();
    let master_decision = decision
        .payload
        .get("note")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("blocked_decision: external action is required")
        .to_owned();
    Ok(Some(ParentBlockedSubtaskTruth {
        task_id: task.task_id.clone(),
        title: task.title.clone(),
        goal: task.goal.clone(),
        status: "blocked".to_owned(),
        blocked_reason,
        blocked_evidence,
        master_decision,
        decision_seq: decision.seq,
    }))
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

fn parent_workset_children(
    task_runtime: &TaskRuntime,
    parent_session_id: &SessionId,
    parent_turn_id: Option<&TurnId>,
) -> Result<Vec<TaskSnapshot>, ProductionMasterRunnerError> {
    let board = task_runtime
        .query_task_board(TaskBoardQuery {
            status: None,
            assignee: None,
            include_terminal: true,
        })
        .map_err(task_center_error)?;
    Ok(board
        .tasks
        .into_iter()
        .filter(|task| {
            task.parent.session_id.as_ref() == Some(parent_session_id)
                && parent_turns_share_logical_group(task.parent.turn_id.as_ref(), parent_turn_id)
        })
        .collect())
}

fn parent_turn_group_key(parent_turn_id: Option<&TurnId>) -> String {
    let Some(parent_turn_id) = parent_turn_id else {
        return "<session>".to_owned();
    };
    let (ordinal, _, raw) = runtime_turn_position(parent_turn_id);
    if ordinal == 0 {
        raw
    } else {
        format!("runtime-turn-{ordinal}")
    }
}

fn parent_turns_share_logical_group(left: Option<&TurnId>, right: Option<&TurnId>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => {
            let (left_ordinal, _, left_raw) = runtime_turn_position(left);
            let (right_ordinal, _, right_raw) = runtime_turn_position(right);
            if left_ordinal != 0 && right_ordinal != 0 {
                left_ordinal == right_ordinal
            } else {
                left_raw == right_raw
            }
        }
        _ => false,
    }
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
    let turns = match persistence.restore_authoritative_turn_snapshots_for_ui(parent_session_id) {
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

fn persisted_parent_blocked_follow_up_summary(
    runtime_home: &Path,
    agent_id: &AgentId,
    parent_session_id: &SessionId,
    evaluation_marker: &str,
) -> Result<Option<String>, ProductionMasterRunnerError> {
    let persistence = ReasonPersistence::new(runtime_home.to_path_buf(), agent_id.clone());
    let turns = match persistence.restore_authoritative_turn_snapshots_for_ui(parent_session_id) {
        Ok(turns) => turns,
        Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => return Ok(None),
        Err(error) => return Err(ProductionMasterRunnerError::State(error.to_string())),
    };
    Ok(turns.into_iter().find_map(|turn| {
        let terminal = turn.terminal_event?;
        (turn.request.user_text.contains(&format!(
            "<freehand_parent_blocked_follow_up id=\"{evaluation_marker}\">"
        )) && matches!(
            terminal.status,
            TerminalStatus::Blocked | TerminalStatus::Failed | TerminalStatus::Cancelled
        ))
        .then_some(terminal.summary)
    }))
}

fn parent_logical_turn_waits_for_lifecycle(
    runtime_home: &Path,
    agent_id: &AgentId,
    parent_session_id: &SessionId,
    parent_turn_id: Option<&TurnId>,
) -> Result<bool, ProductionMasterRunnerError> {
    let Some(parent_turn_id) = parent_turn_id else {
        return Ok(false);
    };
    let persistence = ReasonPersistence::new(runtime_home.to_path_buf(), agent_id.clone());
    let turns = match persistence.restore_authoritative_turn_snapshots_for_ui(parent_session_id) {
        Ok(turns) => turns,
        Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => return Ok(false),
        Err(error) => return Err(ProductionMasterRunnerError::State(error.to_string())),
    };
    let (target_ordinal, _, target_raw) = runtime_turn_position(parent_turn_id);
    let latest = turns
        .iter()
        .filter(|turn| {
            let (ordinal, _, raw) = runtime_turn_position(&turn.request.turn_id);
            if target_ordinal == 0 {
                raw == target_raw
            } else {
                ordinal == target_ordinal
            }
        })
        .max_by_key(|turn| runtime_turn_position(&turn.request.turn_id));
    Ok(latest
        .and_then(|turn| turn.terminal_event.as_ref())
        .is_some_and(|terminal| terminal.status == TerminalStatus::ToolPending))
}

fn next_parent_evaluation_turn_id(
    runtime_home: &Path,
    agent_id: &AgentId,
    parent_session_id: &SessionId,
) -> Result<TurnId, ProductionMasterRunnerError> {
    let persistence = ReasonPersistence::new(runtime_home.to_path_buf(), agent_id.clone());
    let turns = match persistence.restore_authoritative_turn_snapshots_for_ui(parent_session_id) {
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
        attachments: Vec::new(),
        attachment_metadata: Vec::new(),
        cwd: Some(runtime_home.to_path_buf()),
        execution_profile: LiveReasonExecutionProfile::Workspace,
        stream: false,
        cancel_token: None,
    })
}

fn parent_blocked_follow_up_live_request(
    runtime_home: &Path,
    parent_session_id: &SessionId,
    turn_id: &TurnId,
    evaluation_marker: &str,
    user_objectives: &[String],
    blocked_subtasks: &[ParentBlockedSubtaskTruth],
) -> Result<LiveReasonTurnRequest, ProductionMasterRunnerError> {
    let objectives_json = serde_json::to_string_pretty(user_objectives)
        .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
    let blocked_json = serde_json::to_string_pretty(blocked_subtasks)
        .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
    Ok(LiveReasonTurnRequest {
        runtime_home: runtime_home.to_path_buf(),
        session_id: parent_session_id.clone(),
        turn_id: turn_id.clone(),
        trace_id: TraceId::new(format!("master-parent-blocked-trace-{evaluation_marker}")),
        prompt: format!(
            "<freehand_parent_blocked_follow_up id=\"{evaluation_marker}\">\n\
You are the production Master returning to the original user session after a Worker execution was blocked and the blocker was persisted in Task Center truth.\n\
This is a user-visible lifecycle follow-up, not a hidden status note and not a success summary.\n\
Do not claim the objective is complete. Do not create a replacement task unless current Task Center truth permits a concrete correction; the persisted blocked decision says external action is required.\n\
Return an explicit blocked result naming the failed child task, the Worker evidence, and the exact external action required before progress can continue.\n\
Keep the parent session observable and terminal as blocked. Never leave it waiting without a timer or active child execution.\n\
\n\
Original user objective history:\n{objectives_json}\n\
\n\
Blocked child truth:\n{blocked_json}"
        ),
        attachments: Vec::new(),
        attachment_metadata: Vec::new(),
        cwd: Some(runtime_home.to_path_buf()),
        execution_profile: LiveReasonExecutionProfile::Workspace,
        stream: false,
        cancel_token: None,
    })
}

fn parent_user_objectives(
    runtime_home: &Path,
    agent_id: &AgentId,
    parent_session_id: &SessionId,
) -> Result<Option<Vec<String>>, ProductionMasterRunnerError> {
    let persistence = ReasonPersistence::new(runtime_home.to_path_buf(), agent_id.clone());
    let turns = match persistence.restore_turn_start_snapshots(parent_session_id) {
        Ok(turns) => turns,
        Err(ReasonPersistenceError::MissingRecoveryTruth(_)) => return Ok(None),
        Err(error) => return Err(ProductionMasterRunnerError::State(error.to_string())),
    };
    let mut seen = BTreeSet::new();
    let objectives = turns
        .into_iter()
        .filter(|turn| {
            let (_, round, _) = runtime_turn_position(&turn.request.turn_id);
            round == 1
        })
        .map(|turn| ui_user_text_for_turn(&turn))
        .filter(|text| parent_user_objective_text_is_external(text))
        .filter(|text| seen.insert(text.clone()))
        .collect::<Vec<_>>();
    if objectives.is_empty() {
        return Ok(None);
    }
    Ok(Some(objectives))
}

fn parent_user_objective_text_is_external(text: &str) -> bool {
    let text = text.trim();
    !text.is_empty()
        && !text.contains("<freehand_parent_")
        && !text.starts_with(
            "You are the production Master starting a new follow-up turn injected by a due timer.",
        )
        && !text.starts_with("Master attention changed authoritative task truth.")
        && !text.starts_with("The tool result has been returned.")
}

fn timer_live_request(
    runtime_home: &Path,
    agent_id: &AgentId,
    due: &DueTimerSchedule,
) -> Result<LiveReasonTurnRequest, ProductionMasterRunnerError> {
    let timer_json = serde_json::to_string_pretty(&due.schedule)
        .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
    let timer_key = sanitize_identifier(&due.schedule.timer_id);
    let source_session_id = resolve_timer_source_session(runtime_home, agent_id, due)?;
    let session_id = source_session_id
        .clone()
        .unwrap_or_else(|| SessionId::new(format!("master-timer-{timer_key}")));
    let turn_id = if source_session_id.is_some() {
        next_parent_evaluation_turn_id(runtime_home, agent_id, &session_id)?
    } else {
        TurnId::new(format!("master-timer-{timer_key}-fire-{}", due.fired_at))
    };
    Ok(LiveReasonTurnRequest {
        runtime_home: runtime_home.to_path_buf(),
        session_id,
        turn_id,
        trace_id: TraceId::new(format!(
            "master-timer-trace-{timer_key}-fire-{}",
            due.fired_at
        )),
        prompt: format!(
            "You are the production Master starting a new follow-up turn injected by a due timer.\n\
This is a new turn in the source session, not a resume or reopening of the source turn.\n\
Use current framework truth and the injected timer prompt; do not assume task state from memory.\n\
\n\
Timer schedule truth:\n{timer_json}\n\
\n\
Injected timer prompt:\n{}",
            due.schedule.prompt
        ),
        attachments: Vec::new(),
        attachment_metadata: Vec::new(),
        cwd: Some(runtime_home.to_path_buf()),
        execution_profile: LiveReasonExecutionProfile::Workspace,
        stream: false,
        cancel_token: None,
    })
}

fn resolve_timer_source_session(
    runtime_home: &Path,
    agent_id: &AgentId,
    due: &DueTimerSchedule,
) -> Result<Option<SessionId>, ProductionMasterRunnerError> {
    let schedules = super::TimerStore::new(runtime_home, agent_id)
        .load_schedules()
        .map_err(|error| ProductionMasterRunnerError::State(error.to_string()))?;
    let mut current = due.schedule.source_session_id.clone();
    let mut visited = BTreeSet::new();
    while let Some(session_id) = current {
        let Some(internal_key) = session_id.as_str().strip_prefix("master-timer-") else {
            return Ok(Some(session_id));
        };
        if !visited.insert(session_id.clone()) {
            return Err(ProductionMasterRunnerError::State(format!(
                "timer source-session ancestry contains a cycle at `{}`",
                session_id.as_str()
            )));
        }
        let ancestor = schedules
            .iter()
            .find(|schedule| sanitize_identifier(&schedule.timer_id) == internal_key);
        current = ancestor.and_then(|schedule| schedule.source_session_id.clone());
        if ancestor.is_none() {
            return Err(ProductionMasterRunnerError::State(format!(
                "timer source session `{}` has no persisted timer ancestry",
                session_id.as_str()
            )));
        }
    }
    Ok(None)
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

fn master_event_requires_attention(kind: &str) -> bool {
    matches!(
        kind,
        "review_ready"
            | "execution_blocked"
            | "execution_interrupted"
            | "execution_attention_required"
            | "task_closed"
    )
}

fn master_attention_severity_rank(event: &TaskEventInboxEntry) -> u8 {
    match event.kind.as_str() {
        "execution_attention_required" => match event
            .payload
            .get("severity")
            .and_then(serde_json::Value::as_str)
        {
            Some("critical") => 100,
            Some("high") => 98,
            _ => 92,
        },
        "execution_blocked" => 96,
        "execution_interrupted" => 94,
        "review_ready" => 50,
        "task_closed" => 30,
        _ => 0,
    }
}

fn bounded_attention_task_priority(priority: i64) -> i64 {
    priority.clamp(
        MASTER_ATTENTION_TASK_PRIORITY_MIN,
        MASTER_ATTENTION_TASK_PRIORITY_MAX,
    )
}

fn master_attention_effective_score(item: &MasterAttentionItem, next_sequence: u64) -> i128 {
    let age = next_sequence.saturating_sub(item.admitted_sequence);
    master_attention_preemption_score(item)
        + i128::from(age) * MASTER_ATTENTION_ADMISSION_AGE_WEIGHT
}

fn master_attention_preemption_score(item: &MasterAttentionItem) -> i128 {
    i128::from(item.severity_rank) * MASTER_ATTENTION_KIND_WEIGHT
        + i128::from(bounded_attention_task_priority(item.task_priority))
            * MASTER_ATTENTION_TASK_PRIORITY_WEIGHT
}

fn highest_priority_attention_index(
    items: &[MasterAttentionItem],
    next_sequence: u64,
) -> Option<usize> {
    items
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| {
            master_attention_effective_score(left, next_sequence)
                .cmp(&master_attention_effective_score(right, next_sequence))
                .then_with(|| left.severity_rank.cmp(&right.severity_rank))
                .then_with(|| {
                    bounded_attention_task_priority(left.task_priority)
                        .cmp(&bounded_attention_task_priority(right.task_priority))
                })
                .then_with(|| right.admitted_sequence.cmp(&left.admitted_sequence))
                .then_with(|| right.event.event_id.cmp(&left.event.event_id))
        })
        .map(|(index, _)| index)
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
