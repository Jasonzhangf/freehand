use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use freehand_config::{AgentMode, SelectedAgentConfig};
use freehand_contracts::{AgentId, SessionId, TerminalStatus, TraceId, TurnId};
use freehand_task::{
    AgentCreateRequest, ExecutionFact, ExecutionFactKind, TaskActor, TaskAssignRequest,
    TaskClaimOutcome, TaskClaimRequest, TaskError, TaskId, TaskListQuery, TaskRuntime,
    TaskSnapshot, TaskStatus, TaskWatermark,
};
use thiserror::Error;

use super::{
    LiveReasonTurnRequest, RuntimeAgentBootstrapError, load_default_runtime_agent,
    run_worker_live_reason_turn,
};

mod heartbeat;
#[cfg(test)]
mod tests;

use heartbeat::WorkerHeartbeat;

const DEFAULT_LEASE_TTL_SECONDS: u64 = 30;
const DEFAULT_POLL_INTERVAL_MILLIS: u64 = 1_000;

static EXECUTION_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductionWorkerTickOutcome {
    Idle,
    ReviewReady {
        task_id: TaskId,
        execution_id: String,
        turn_id: TurnId,
        summary: String,
    },
    Blocked {
        task_id: TaskId,
        execution_id: String,
        turn_id: Option<TurnId>,
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WorkerRetryKind {
    Interrupted,
    ReviewRejected,
}

#[derive(Debug, Error)]
pub enum ProductionWorkerRunnerError {
    #[error("worker bootstrap failed: {0}")]
    Bootstrap(#[from] RuntimeAgentBootstrapError),
    #[error("worker runner requires a slave agent, got `{mode}` for `{agent_name}`")]
    RequiresSlaveMode { agent_name: String, mode: String },
    #[error("worker runner requires a paired master, got `{mode}` for `{agent_name}`")]
    RequiresPairedMaster { agent_name: String, mode: String },
    #[error("worker Task Center failed: {0}")]
    TaskCenter(String),
    #[error("worker heartbeat failed: {0}")]
    Heartbeat(String),
    #[error("worker execution failed and blocked fact could not be persisted: {0}")]
    BlockedFactPersistence(String),
}

#[derive(Debug, Clone)]
struct WorkerTurnExecution {
    status: TerminalStatus,
    summary: String,
    turn_id: TurnId,
}

trait WorkerTurnExecutor: Send + Sync {
    fn execute(
        &self,
        selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
    ) -> Result<WorkerTurnExecution, String>;
}

struct LiveWorkerTurnExecutor;

impl WorkerTurnExecutor for LiveWorkerTurnExecutor {
    fn execute(
        &self,
        selected: &SelectedAgentConfig,
        request: LiveReasonTurnRequest,
    ) -> Result<WorkerTurnExecution, String> {
        let outcome =
            run_worker_live_reason_turn(selected, request).map_err(|error| error.to_string())?;
        let terminal = outcome
            .turn
            .terminal_event
            .as_ref()
            .ok_or_else(|| "worker live turn closed without terminal event".to_owned())?;
        Ok(WorkerTurnExecution {
            status: terminal.status.clone(),
            summary: terminal.summary.clone(),
            turn_id: outcome.turn.request.turn_id.clone(),
        })
    }
}

pub struct ProductionWorkerRunner {
    selected: SelectedAgentConfig,
    runtime_home: PathBuf,
    task_owner_agent_id: AgentId,
    worker_agent_id: AgentId,
    executor: Arc<dyn WorkerTurnExecutor>,
}

impl ProductionWorkerRunner {
    pub fn from_default_config(agent_name: &str) -> Result<Self, ProductionWorkerRunnerError> {
        let bootstrap = load_default_runtime_agent(agent_name)?;
        Self::from_selected_agent(bootstrap.selected_agent, bootstrap.runtime_home)
    }

    pub fn from_selected_agent(
        selected: SelectedAgentConfig,
        runtime_home: PathBuf,
    ) -> Result<Self, ProductionWorkerRunnerError> {
        Self::from_selected_agent_with_executor(
            selected,
            runtime_home,
            Arc::new(LiveWorkerTurnExecutor),
        )
    }

    fn from_selected_agent_with_executor(
        selected: SelectedAgentConfig,
        runtime_home: PathBuf,
        executor: Arc<dyn WorkerTurnExecutor>,
    ) -> Result<Self, ProductionWorkerRunnerError> {
        if selected.mode != AgentMode::Slave {
            return Err(ProductionWorkerRunnerError::RequiresSlaveMode {
                agent_name: selected.name.clone(),
                mode: selected.mode.as_str().to_owned(),
            });
        }
        if selected.paired_agent_mode != AgentMode::Master {
            return Err(ProductionWorkerRunnerError::RequiresPairedMaster {
                agent_name: selected.paired_agent_name.clone(),
                mode: selected.paired_agent_mode.as_str().to_owned(),
            });
        }
        fs::create_dir_all(&runtime_home)
            .map_err(|error| ProductionWorkerRunnerError::TaskCenter(error.to_string()))?;
        let runner = Self {
            task_owner_agent_id: AgentId::new(selected.paired_agent_name.clone()),
            worker_agent_id: AgentId::new(selected.name.clone()),
            selected,
            runtime_home,
            executor,
        };
        runner.ensure_worker_registered()?;
        Ok(runner)
    }

    pub fn run_once(&self) -> Result<ProductionWorkerTickOutcome, ProductionWorkerRunnerError> {
        let task_runtime = Arc::new(self.open_task_center()?);
        self.ensure_worker_registered_in(&task_runtime)?;
        let execution_id = next_execution_id(&self.worker_agent_id);
        let mut retry_kind = None;
        let mut claim = self.claim_assigned_task(&task_runtime, &execution_id)?;
        if claim.task.is_none() {
            retry_kind = self.requeue_retryable_task(&task_runtime)?;
            if retry_kind.is_some() {
                claim = self.claim_assigned_task(&task_runtime, &execution_id)?;
            }
        }
        let Some(task) = claim.task else {
            return Ok(ProductionWorkerTickOutcome::Idle);
        };
        let execution_id = claim.execution_id.ok_or_else(|| {
            ProductionWorkerRunnerError::TaskCenter(
                "claimed task did not return an execution id".to_owned(),
            )
        })?;

        let Some(target_cwd) = task.target_cwd.as_deref() else {
            return self.report_blocked(
                &task_runtime,
                &task,
                &execution_id,
                None,
                "assigned worker task is missing target_cwd".to_owned(),
            );
        };
        let workspace = match canonical_worker_workspace(target_cwd) {
            Ok(workspace) => workspace,
            Err(reason) => {
                return self.report_blocked(
                    &task_runtime,
                    &task,
                    &execution_id,
                    None,
                    reason.into_reason(),
                );
            }
        };

        let heartbeat = WorkerHeartbeat::start(
            Arc::clone(&task_runtime),
            task.task_id.clone(),
            self.worker_agent_id.clone(),
            execution_id.clone(),
        );
        let request = worker_live_request(
            &self.runtime_home,
            &task,
            &execution_id,
            workspace,
            retry_kind,
        );
        let execution = self.executor.execute(&self.selected, request);
        if let Err(error) = heartbeat.stop() {
            return self.report_blocked(
                &task_runtime,
                &task,
                &execution_id,
                None,
                error.to_string(),
            );
        }

        match execution {
            Ok(execution) if execution.status == TerminalStatus::Success => {
                let summary = execution.summary.clone();
                task_runtime
                    .apply_execution_fact(ExecutionFact {
                        execution_id: execution_id.clone(),
                        task_id: task.task_id.clone(),
                        agent_id: self.worker_agent_id.clone(),
                        turn_id: Some(execution.turn_id.clone()),
                        occurred_at: now_unix_seconds(),
                        kind: ExecutionFactKind::ReviewReady {
                            summary: summary.clone(),
                            deliverables: task.deliverables.clone(),
                            evidence: vec![
                                format!("worker_turn_id={}", execution.turn_id.as_str()),
                                format!("target_cwd={target_cwd}"),
                            ],
                        },
                        watermark: worker_watermark(&execution_id, "review_ready"),
                    })
                    .map_err(task_center_error)?;
                Ok(ProductionWorkerTickOutcome::ReviewReady {
                    task_id: task.task_id,
                    execution_id,
                    turn_id: execution.turn_id,
                    summary,
                })
            }
            Ok(execution) => self.report_blocked(
                &task_runtime,
                &task,
                &execution_id,
                Some(execution.turn_id),
                format!(
                    "worker turn ended with status {:?}: {}",
                    execution.status, execution.summary
                ),
            ),
            Err(reason) => self.report_blocked(
                &task_runtime,
                &task,
                &execution_id,
                None,
                format!("worker live execution failed: {reason}"),
            ),
        }
    }

    pub fn run(&self) -> Result<(), ProductionWorkerRunnerError> {
        let interval = Duration::from_millis(DEFAULT_POLL_INTERVAL_MILLIS);
        loop {
            match self.run_once()? {
                ProductionWorkerTickOutcome::Idle => {}
                outcome => println!("worker runner outcome: {outcome:?}"),
            }
            thread::sleep(interval);
        }
    }

    fn open_task_center(&self) -> Result<TaskRuntime, ProductionWorkerRunnerError> {
        TaskRuntime::boot(&self.runtime_home, self.task_owner_agent_id.clone())
            .map_err(task_center_error)
    }

    fn claim_assigned_task(
        &self,
        task_runtime: &TaskRuntime,
        execution_id: &str,
    ) -> Result<TaskClaimOutcome, ProductionWorkerRunnerError> {
        task_runtime
            .claim_next_task(TaskClaimRequest {
                agent_id: self.worker_agent_id.clone(),
                execution_id: execution_id.to_owned(),
                ttl_seconds: DEFAULT_LEASE_TTL_SECONDS,
                actor: worker_actor(&self.worker_agent_id, None),
                watermark: worker_watermark(execution_id, "claim"),
            })
            .map_err(task_center_error)
    }

    fn requeue_retryable_task(
        &self,
        task_runtime: &TaskRuntime,
    ) -> Result<Option<WorkerRetryKind>, ProductionWorkerRunnerError> {
        let retryable = task_runtime
            .list_tasks(TaskListQuery {
                status: None,
                assignee: Some(self.worker_agent_id.clone()),
            })
            .map_err(task_center_error)?
            .into_iter()
            .find_map(|task| match task.status {
                TaskStatus::Interrupted => Some((task, WorkerRetryKind::Interrupted)),
                TaskStatus::Rejected => Some((task, WorkerRetryKind::ReviewRejected)),
                _ => None,
            });
        let Some((task, retry_kind)) = retryable else {
            return Ok(None);
        };
        task_runtime
            .assign_task(TaskAssignRequest {
                task_id: task.task_id,
                agent_id: self.worker_agent_id.clone(),
                actor: worker_actor(&self.worker_agent_id, None),
                watermark: worker_watermark("retry", "requeue"),
            })
            .map_err(task_center_error)?;
        Ok(Some(retry_kind))
    }

    fn ensure_worker_registered(&self) -> Result<(), ProductionWorkerRunnerError> {
        let task_runtime = self.open_task_center()?;
        self.ensure_worker_registered_in(&task_runtime)
    }

    fn ensure_worker_registered_in(
        &self,
        task_runtime: &TaskRuntime,
    ) -> Result<(), ProductionWorkerRunnerError> {
        match task_runtime.query_agent(&self.worker_agent_id) {
            Ok(_) => Ok(()),
            Err(TaskError::AgentNotFound(_)) => {
                task_runtime
                    .create_agent(AgentCreateRequest {
                        agent_id: self.worker_agent_id.clone(),
                        capabilities: vec![
                            "workspace".to_owned(),
                            "shell".to_owned(),
                            "provider".to_owned(),
                        ],
                        actor: worker_actor(&self.worker_agent_id, None),
                        watermark: worker_watermark("bootstrap", "register"),
                    })
                    .map_err(task_center_error)?;
                Ok(())
            }
            Err(error) => Err(task_center_error(error)),
        }
    }

    fn report_blocked(
        &self,
        task_runtime: &TaskRuntime,
        task: &TaskSnapshot,
        execution_id: &str,
        turn_id: Option<TurnId>,
        reason: String,
    ) -> Result<ProductionWorkerTickOutcome, ProductionWorkerRunnerError> {
        task_runtime
            .apply_execution_fact(ExecutionFact {
                execution_id: execution_id.to_owned(),
                task_id: task.task_id.clone(),
                agent_id: self.worker_agent_id.clone(),
                turn_id: turn_id.clone(),
                occurred_at: now_unix_seconds(),
                kind: ExecutionFactKind::Blocked {
                    reason: reason.clone(),
                    evidence: vec![reason.clone()],
                },
                watermark: worker_watermark(execution_id, "blocked"),
            })
            .map_err(|error| {
                ProductionWorkerRunnerError::BlockedFactPersistence(error.to_string())
            })?;
        Ok(ProductionWorkerTickOutcome::Blocked {
            task_id: task.task_id.clone(),
            execution_id: execution_id.to_owned(),
            turn_id,
            reason,
        })
    }
}

fn worker_live_request(
    runtime_home: &Path,
    task: &TaskSnapshot,
    execution_id: &str,
    workspace: PathBuf,
    retry_kind: Option<WorkerRetryKind>,
) -> LiveReasonTurnRequest {
    let task_key = sanitize_identifier(task.task_id.as_str());
    let execution_key = sanitize_identifier(execution_id);
    let prompt = worker_task_prompt(task, &workspace, retry_kind);
    LiveReasonTurnRequest {
        runtime_home: runtime_home.to_path_buf(),
        session_id: SessionId::new(format!("worker-task-{task_key}")),
        turn_id: TurnId::new(format!("worker-turn-{execution_key}")),
        trace_id: TraceId::new(format!("worker-trace-{execution_key}")),
        prompt,
        cwd: Some(workspace),
        stream: false,
        cancel_token: None,
    }
}

fn worker_task_prompt(
    task: &TaskSnapshot,
    canonical_workspace: &Path,
    retry_kind: Option<WorkerRetryKind>,
) -> String {
    let retry_context = match retry_kind {
        Some(WorkerRetryKind::Interrupted) => {
            "\nRetry context:\nThe previous execution was interrupted. Inspect persisted workspace state, continue safely, and re-run verification before submission.".to_owned()
        }
        Some(WorkerRetryKind::ReviewRejected) => format!(
            "\nReview rejection:\nReason: {}\nRequired changes:\n{}",
            task.review.reject_reason.as_deref().unwrap_or("not provided"),
            render_lines(&task.review.next_requirements),
        ),
        None => String::new(),
    };
    format!(
        "Execute the assigned Task Center task.\n\
Task ID: {}\n\
Title: {}\n\
Goal: {}\n\
	Content: {}\n\
	Requested target_cwd: {}\n\
	Canonical locked workspace: {}\n\
	Deliverables:\n{}\n\
	Acceptance criteria:\n{}\n\
	Path preflight:\n\
	- Treat the requested target_cwd as the user-facing path and the canonical locked workspace as execution truth.\n\
	- Before path-sensitive work, verify whether relevant paths are absolute, whether they contain a leading ~, and whether the requested path or any parent is a symlink.\n\
	- If a symlink is present, report both requested and canonical paths in evidence.\n\
	- If a required path is missing, block with the exact path and canonicalization error; do not invent alternate directories.\n\
	Work only inside the locked target workspace. Complete the implementation and verification, then return the required Freehand completion schema.{}",
        task.task_id.as_str(),
        task.title,
        task.goal,
        task.content,
        task.target_cwd.as_deref().unwrap_or("(missing)"),
        canonical_workspace.display(),
        render_lines(&task.deliverables),
        render_lines(&task.acceptance),
        retry_context,
    )
}

fn render_lines(values: &[String]) -> String {
    if values.is_empty() {
        return "- none specified".to_owned();
    }
    values
        .iter()
        .map(|value| format!("- {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

enum WorkerWorkspacePreflightError {
    Empty,
    MissingWorkspace {
        requested: String,
        expanded: PathBuf,
        parent_exists: bool,
    },
    PermissionDenied {
        requested: String,
        expanded: PathBuf,
        error: String,
    },
    CanonicalizeFailed {
        requested: String,
        expanded: PathBuf,
        error: String,
    },
    NotDirectory {
        canonical: PathBuf,
    },
}

impl WorkerWorkspacePreflightError {
    fn into_reason(self) -> String {
        match self {
            Self::Empty => "assigned worker task target_cwd is empty".to_owned(),
            Self::MissingWorkspace {
                requested,
                expanded,
                parent_exists,
            } => {
                if parent_exists {
                    format!(
                        "worker task workspace preflight failed: target_cwd `{requested}` expands to `{}` but that workspace path does not exist. This is not a repository-permission denial. It usually means the task used target_cwd for a not-yet-created output directory or the wrong workspace root. target_cwd must point to an existing workspace/repository; create output directories later from inside that workspace.",
                        expanded.display()
                    )
                } else {
                    format!(
                        "worker task workspace preflight failed: target_cwd `{requested}` expands to `{}` but the path cannot be resolved because one of its parent directories does not exist. This is not proof that the intended repository is unavailable; it means the assigned workspace path itself is invalid and must be corrected before execution.",
                        expanded.display()
                    )
                }
            }
            Self::PermissionDenied {
                requested,
                expanded,
                error,
            } => format!(
                "worker task workspace preflight failed: target_cwd `{requested}` expands to `{}` but access was denied during workspace resolution: {error}. This is a path-access or boundary problem, not evidence that the repository is missing.",
                expanded.display()
            ),
            Self::CanonicalizeFailed {
                requested,
                expanded,
                error,
            } => format!(
                "worker task workspace preflight failed: target_cwd `{requested}` expands to `{}` but canonical workspace resolution failed: {error}.",
                expanded.display()
            ),
            Self::NotDirectory { canonical } => format!(
                "worker task workspace preflight failed: canonical target_cwd `{}` is not a directory",
                canonical.display()
            ),
        }
    }
}

fn canonical_worker_workspace(target_cwd: &str) -> Result<PathBuf, WorkerWorkspacePreflightError> {
    let trimmed = target_cwd.trim();
    if trimmed.is_empty() {
        return Err(WorkerWorkspacePreflightError::Empty);
    }
    let expanded = expand_leading_tilde(trimmed);
    let workspace = fs::canonicalize(&expanded).map_err(|error| match error.kind() {
        ErrorKind::NotFound => WorkerWorkspacePreflightError::MissingWorkspace {
            requested: target_cwd.to_owned(),
            parent_exists: expanded.parent().is_some_and(Path::exists),
            expanded: expanded.clone(),
        },
        ErrorKind::PermissionDenied => WorkerWorkspacePreflightError::PermissionDenied {
            requested: target_cwd.to_owned(),
            expanded: expanded.clone(),
            error: error.to_string(),
        },
        _ => WorkerWorkspacePreflightError::CanonicalizeFailed {
            requested: target_cwd.to_owned(),
            expanded: expanded.clone(),
            error: error.to_string(),
        },
    })?;
    if !workspace.is_dir() {
        return Err(WorkerWorkspacePreflightError::NotDirectory {
            canonical: workspace,
        });
    }
    Ok(workspace)
}

fn expand_leading_tilde(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(rest);
    }
    PathBuf::from(path)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

fn next_execution_id(worker_agent_id: &AgentId) -> String {
    let counter = EXECUTION_COUNTER.fetch_add(1, Ordering::Relaxed);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    format!(
        "exec-worker-{}-{nanos}-{counter}",
        sanitize_identifier(worker_agent_id.as_str())
    )
}

fn sanitize_identifier(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character
            } else {
                '-'
            }
        })
        .collect()
}

fn worker_actor(worker_agent_id: &AgentId, turn_id: Option<TurnId>) -> TaskActor {
    TaskActor {
        agent_id: worker_agent_id.clone(),
        source: "runtime.master-worker-loop".to_owned(),
        session_id: None,
        turn_id,
        trace_id: None,
    }
}

fn worker_watermark(execution_id: &str, stage: &str) -> TaskWatermark {
    TaskWatermark {
        metadata_id: None,
        hook: Some(format!("runtime.master-worker-loop.{stage}")),
        action_tool_call_id: Some(execution_id.to_owned()),
    }
}

fn task_center_error(error: TaskError) -> ProductionWorkerRunnerError {
    ProductionWorkerRunnerError::TaskCenter(error.to_string())
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_secs()
}
