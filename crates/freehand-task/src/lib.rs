//! Task orchestration truth, persistence, startup recovery, and agent registry skeleton.

use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use freehand_contracts::{AgentId, SessionId, TraceId, TurnId};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TaskId(String);

impl TaskId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Created,
    WaitingAgent,
    Assigned,
    Running,
    Paused,
    Blocked,
    ReviewSubmitted,
    Approved,
    Rejected,
    Failed,
    Cancelled,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentStatus {
    Available,
    Busy,
    Paused,
    Offline,
    Closing,
    Closed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskAssignee {
    pub agent_id: AgentId,
    pub assignment_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskReviewState {
    pub status: String,
    pub submitted_at: Option<u64>,
    pub reviewer_agent_id: Option<AgentId>,
    pub decision: Option<String>,
    pub reject_reason: Option<String>,
    pub next_requirements: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskParentRef {
    pub session_id: Option<SessionId>,
    pub turn_id: Option<TurnId>,
    pub trace_id: Option<TraceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskSnapshot {
    pub schema_version: u32,
    pub task_id: TaskId,
    pub status: TaskStatus,
    pub title: String,
    pub content: String,
    pub goal: String,
    pub deliverables: Vec<String>,
    pub acceptance: Vec<String>,
    pub priority: i64,
    pub target_cwd: Option<String>,
    pub assignee: Option<TaskAssignee>,
    pub review: TaskReviewState,
    pub parent: TaskParentRef,
    pub created_at: u64,
    pub updated_at: u64,
    pub last_progress_at: Option<u64>,
    pub last_event_seq: u64,
    pub last_event_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSnapshot {
    pub schema_version: u32,
    pub agent_id: AgentId,
    pub status: AgentStatus,
    pub current_task_id: Option<TaskId>,
    pub current_cwd: Option<String>,
    pub capabilities: Vec<String>,
    pub last_seen_at: u64,
    pub running_tasks: u32,
    pub queued_tasks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskLedgerEvent {
    pub schema_version: u32,
    pub task_id: TaskId,
    pub seq: u64,
    pub event_id: String,
    pub event_type: String,
    pub from_status: Option<TaskStatus>,
    pub to_status: TaskStatus,
    pub timestamp: u64,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
    pub payload: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskActor {
    pub agent_id: AgentId,
    pub source: String,
    pub session_id: Option<SessionId>,
    pub turn_id: Option<TurnId>,
    pub trace_id: Option<TraceId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskWatermark {
    pub metadata_id: Option<String>,
    pub hook: Option<String>,
    pub action_tool_call_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCreateRequest {
    pub task_id: Option<TaskId>,
    pub title: String,
    pub content: String,
    pub goal: String,
    pub deliverables: Vec<String>,
    pub acceptance: Vec<String>,
    pub priority: i64,
    pub target_cwd: Option<String>,
    pub dispatch: TaskDispatchRequest,
    pub parent: TaskParentRef,
    pub actor: TaskActor,
    pub watermark: TaskWatermark,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum TaskDispatchRequest {
    None,
    SelfAgent,
    Agent { agent_id: AgentId },
    Auto { allow_create_agent: bool },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskCreateOutcome {
    pub task: TaskSnapshot,
    pub events: Vec<TaskLedgerEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskRuntimeSnapshot {
    pub status: String,
    pub tasks: Vec<TaskSnapshot>,
    pub agents: Vec<AgentSnapshot>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TaskError {
    #[error("task field `{0}` is required")]
    MissingField(&'static str),
    #[error("task `{0}` already exists")]
    TaskAlreadyExists(String),
    #[error("task `{0}` not found")]
    TaskNotFound(String),
    #[error("agent `{0}` not found")]
    AgentNotFound(String),
    #[error("task persistence failed: {0}")]
    Persistence(String),
    #[error("task ledger replay failed: {0}")]
    Replay(String),
}

pub struct TaskRuntime {
    store: TaskStore,
    state: Mutex<TaskRuntimeState>,
}

#[derive(Debug, Clone, Default)]
struct TaskRuntimeState {
    tasks: BTreeMap<TaskId, TaskSnapshot>,
    agents: BTreeMap<AgentId, AgentSnapshot>,
}

impl TaskRuntime {
    pub fn boot(
        runtime_home: impl Into<PathBuf>,
        owner_agent_id: AgentId,
    ) -> Result<Self, TaskError> {
        let store = TaskStore::new(runtime_home, owner_agent_id.clone());
        let mut state = TaskRuntimeState::default();
        let self_agent = store.load_or_create_self_agent(&owner_agent_id)?;
        state.agents.insert(owner_agent_id, self_agent);
        for task in store.load_task_snapshots()? {
            state.tasks.insert(task.task_id.clone(), task);
        }
        Ok(Self {
            store,
            state: Mutex::new(state),
        })
    }

    pub fn create_task(&self, request: TaskCreateRequest) -> Result<TaskCreateOutcome, TaskError> {
        validate_create_request(&request)?;
        let mut state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        let task_id = request
            .task_id
            .clone()
            .unwrap_or_else(|| TaskId::new(format!("task-{}", now_unix_seconds())));
        if state.tasks.contains_key(&task_id) || self.store.task_snapshot_path(&task_id).is_file() {
            return Err(TaskError::TaskAlreadyExists(task_id.as_str().to_owned()));
        }
        let created_at = now_unix_seconds();
        let mut task = TaskSnapshot {
            schema_version: 1,
            task_id: task_id.clone(),
            status: TaskStatus::Created,
            title: request.title,
            content: request.content,
            goal: request.goal,
            deliverables: request.deliverables,
            acceptance: request.acceptance,
            priority: request.priority,
            target_cwd: request.target_cwd,
            assignee: None,
            review: TaskReviewState {
                status: "none".to_owned(),
                submitted_at: None,
                reviewer_agent_id: None,
                decision: None,
                reject_reason: None,
                next_requirements: Vec::new(),
            },
            parent: request.parent,
            created_at,
            updated_at: created_at,
            last_progress_at: None,
            last_event_seq: 0,
            last_event_id: String::new(),
        };
        let mut events = Vec::new();
        let created = build_event(
            &task,
            None,
            TaskStatus::Created,
            "TaskCreated",
            &request.actor,
            &request.watermark,
            json!({}),
        );
        apply_event(&mut task, &created);
        self.store.append_event_and_snapshot(&task, &created)?;
        events.push(created);

        match resolve_dispatch(&request.dispatch, &state.agents)? {
            Some(agent_id) => {
                let from = task.status.clone();
                task.assignee = Some(TaskAssignee {
                    agent_id: agent_id.clone(),
                    assignment_id: format!("assign-{}-{}", task.task_id.as_str(), events.len() + 1),
                });
                task.status = TaskStatus::Assigned;
                let assigned = build_event(
                    &task,
                    Some(from),
                    TaskStatus::Assigned,
                    "TaskAssigned",
                    &request.actor,
                    &request.watermark,
                    json!({"agent_id": agent_id.as_str()}),
                );
                apply_event(&mut task, &assigned);
                self.store.append_event_and_snapshot(&task, &assigned)?;
                events.push(assigned);
                if let Some(agent) = state.agents.get_mut(&agent_id) {
                    agent.status = AgentStatus::Busy;
                    agent.current_task_id = Some(task.task_id.clone());
                    agent.current_cwd = task.target_cwd.clone();
                    agent.running_tasks = 1;
                    agent.last_seen_at = now_unix_seconds();
                    self.store.write_agent_snapshot(agent)?;
                }
            }
            None => {
                let from = task.status.clone();
                task.status = TaskStatus::WaitingAgent;
                let waiting = build_event(
                    &task,
                    Some(from),
                    TaskStatus::WaitingAgent,
                    "TaskWaitingAgent",
                    &request.actor,
                    &request.watermark,
                    json!({}),
                );
                apply_event(&mut task, &waiting);
                self.store.append_event_and_snapshot(&task, &waiting)?;
                events.push(waiting);
            }
        }
        state.tasks.insert(task.task_id.clone(), task.clone());
        Ok(TaskCreateOutcome { task, events })
    }

    pub fn query_task(&self, task_id: &TaskId) -> Result<TaskSnapshot, TaskError> {
        self.state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?
            .tasks
            .get(task_id)
            .cloned()
            .ok_or_else(|| TaskError::TaskNotFound(task_id.as_str().to_owned()))
    }

    pub fn list_agents(&self) -> Result<Vec<AgentSnapshot>, TaskError> {
        Ok(self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?
            .agents
            .values()
            .cloned()
            .collect())
    }

    pub fn query_agent(&self, agent_id: &AgentId) -> Result<AgentSnapshot, TaskError> {
        self.state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?
            .agents
            .get(agent_id)
            .cloned()
            .ok_or_else(|| TaskError::AgentNotFound(agent_id.as_str().to_owned()))
    }

    pub fn snapshot(&self) -> Result<TaskRuntimeSnapshot, TaskError> {
        let state = self
            .state
            .lock()
            .map_err(|err| TaskError::Persistence(err.to_string()))?;
        Ok(TaskRuntimeSnapshot {
            status: "ready".to_owned(),
            tasks: state.tasks.values().cloned().collect(),
            agents: state.agents.values().cloned().collect(),
        })
    }
}

#[derive(Debug, Clone)]
struct TaskStore {
    runtime_home: PathBuf,
    owner_agent_id: AgentId,
}

impl TaskStore {
    fn new(runtime_home: impl Into<PathBuf>, owner_agent_id: AgentId) -> Self {
        Self {
            runtime_home: runtime_home.into(),
            owner_agent_id,
        }
    }

    fn load_or_create_self_agent(
        &self,
        owner_agent_id: &AgentId,
    ) -> Result<AgentSnapshot, TaskError> {
        let path = self.agent_snapshot_path(owner_agent_id);
        if path.is_file() {
            return read_json(&path);
        }
        let now = now_unix_seconds();
        let snapshot = AgentSnapshot {
            schema_version: 1,
            agent_id: owner_agent_id.clone(),
            status: AgentStatus::Available,
            current_task_id: None,
            current_cwd: None,
            capabilities: vec![
                "code_edit".to_owned(),
                "test_run".to_owned(),
                "docs".to_owned(),
            ],
            last_seen_at: now,
            running_tasks: 0,
            queued_tasks: 0,
        };
        self.write_agent_snapshot(&snapshot)?;
        Ok(snapshot)
    }

    fn load_task_snapshots(&self) -> Result<Vec<TaskSnapshot>, TaskError> {
        let task_dir = self.task_state_dir();
        if !task_dir.is_dir() {
            return Ok(Vec::new());
        }
        let mut tasks = Vec::new();
        for entry in fs::read_dir(&task_dir).map_err(io_err)? {
            let path = entry.map_err(io_err)?.path();
            if path.extension().and_then(|ext| ext.to_str()) == Some("json")
                && path.file_name().and_then(|name| name.to_str()) != Some("index.json")
            {
                tasks.push(read_json(&path)?);
            }
        }
        Ok(tasks)
    }

    fn append_event_and_snapshot(
        &self,
        snapshot: &TaskSnapshot,
        event: &TaskLedgerEvent,
    ) -> Result<(), TaskError> {
        let ledger_path = self.task_ledger_path(&snapshot.task_id);
        ensure_parent_dir(&ledger_path)?;
        let line = serde_json::to_string(event).map_err(json_err)?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&ledger_path)
            .map_err(io_err)?;
        writeln!(file, "{line}").map_err(io_err)?;
        write_json_atomic(&self.task_snapshot_path(&snapshot.task_id), snapshot)?;
        self.write_task_index()
    }

    fn write_task_index(&self) -> Result<(), TaskError> {
        let tasks = self
            .load_task_snapshots()?
            .into_iter()
            .map(|task| task.task_id)
            .collect::<Vec<_>>();
        write_json_atomic(&self.task_state_dir().join("index.json"), &tasks)
    }

    fn write_agent_snapshot(&self, snapshot: &AgentSnapshot) -> Result<(), TaskError> {
        write_json_atomic(&self.agent_snapshot_path(&snapshot.agent_id), snapshot)?;
        self.write_agent_index()
    }

    fn write_agent_index(&self) -> Result<(), TaskError> {
        let dir = self.agent_state_dir();
        let mut agents = Vec::<AgentId>::new();
        if dir.is_dir() {
            for entry in fs::read_dir(&dir).map_err(io_err)? {
                let path = entry.map_err(io_err)?.path();
                if path.extension().and_then(|ext| ext.to_str()) == Some("json")
                    && path.file_name().and_then(|name| name.to_str()) != Some("index.json")
                {
                    let agent: AgentSnapshot = read_json(&path)?;
                    agents.push(agent.agent_id);
                }
            }
        }
        write_json_atomic(&dir.join("index.json"), &agents)
    }

    fn task_state_dir(&self) -> PathBuf {
        self.runtime_home
            .join("state")
            .join("tasks")
            .join(self.owner_agent_id.as_str())
    }

    fn task_ledger_dir(&self) -> PathBuf {
        self.runtime_home
            .join("ledgers")
            .join("tasks")
            .join(self.owner_agent_id.as_str())
    }

    fn agent_state_dir(&self) -> PathBuf {
        self.runtime_home.join("state").join("agents")
    }

    fn task_snapshot_path(&self, task_id: &TaskId) -> PathBuf {
        self.task_state_dir()
            .join(format!("{}.json", task_id.as_str()))
    }

    fn task_ledger_path(&self, task_id: &TaskId) -> PathBuf {
        self.task_ledger_dir()
            .join(format!("{}.jsonl", task_id.as_str()))
    }

    fn agent_snapshot_path(&self, agent_id: &AgentId) -> PathBuf {
        self.agent_state_dir()
            .join(format!("{}.json", agent_id.as_str()))
    }
}

fn resolve_dispatch(
    dispatch: &TaskDispatchRequest,
    agents: &BTreeMap<AgentId, AgentSnapshot>,
) -> Result<Option<AgentId>, TaskError> {
    match dispatch {
        TaskDispatchRequest::None => Ok(None),
        TaskDispatchRequest::SelfAgent => Ok(agents
            .values()
            .find(|agent| matches!(agent.status, AgentStatus::Available))
            .map(|agent| agent.agent_id.clone())),
        TaskDispatchRequest::Agent { agent_id } => {
            let agent = agents
                .get(agent_id)
                .ok_or_else(|| TaskError::AgentNotFound(agent_id.as_str().to_owned()))?;
            if matches!(agent.status, AgentStatus::Available) {
                Ok(Some(agent_id.clone()))
            } else {
                Ok(None)
            }
        }
        TaskDispatchRequest::Auto { .. } => Ok(agents
            .values()
            .find(|agent| matches!(agent.status, AgentStatus::Available))
            .map(|agent| agent.agent_id.clone())),
    }
}

fn validate_create_request(request: &TaskCreateRequest) -> Result<(), TaskError> {
    require_text(&request.title, "title")?;
    require_text(&request.content, "content")?;
    require_text(&request.goal, "goal")?;
    if request.deliverables.is_empty() {
        return Err(TaskError::MissingField("deliverables"));
    }
    if request.acceptance.is_empty() {
        return Err(TaskError::MissingField("acceptance"));
    }
    Ok(())
}

fn require_text(value: &str, field: &'static str) -> Result<(), TaskError> {
    if value.trim().is_empty() {
        Err(TaskError::MissingField(field))
    } else {
        Ok(())
    }
}

fn build_event(
    snapshot: &TaskSnapshot,
    from_status: Option<TaskStatus>,
    to_status: TaskStatus,
    event_type: &str,
    actor: &TaskActor,
    watermark: &TaskWatermark,
    payload: Value,
) -> TaskLedgerEvent {
    let seq = snapshot.last_event_seq.saturating_add(1);
    TaskLedgerEvent {
        schema_version: 1,
        task_id: snapshot.task_id.clone(),
        seq,
        event_id: format!("{}:{seq}", snapshot.task_id.as_str()),
        event_type: event_type.to_owned(),
        from_status,
        to_status,
        timestamp: now_unix_seconds(),
        actor: actor.clone(),
        watermark: watermark.clone(),
        payload,
    }
}

fn apply_event(snapshot: &mut TaskSnapshot, event: &TaskLedgerEvent) {
    snapshot.status = event.to_status.clone();
    snapshot.updated_at = event.timestamp;
    snapshot.last_event_seq = event.seq;
    snapshot.last_event_id = event.event_id.clone();
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, TaskError> {
    let raw = fs::read_to_string(path).map_err(io_err)?;
    serde_json::from_str(&raw).map_err(json_err)
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), TaskError> {
    ensure_parent_dir(path)?;
    let temp = path.with_extension(format!("tmp-{}", now_unix_seconds()));
    let raw = serde_json::to_string_pretty(value).map_err(json_err)?;
    fs::write(&temp, raw).map_err(io_err)?;
    fs::rename(&temp, path).map_err(io_err)
}

fn ensure_parent_dir(path: &Path) -> Result<(), TaskError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io_err)?;
    }
    Ok(())
}

fn now_unix_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn io_err(err: std::io::Error) -> TaskError {
    TaskError::Persistence(err.to_string())
}

fn json_err(err: serde_json::Error) -> TaskError {
    TaskError::Persistence(err.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_task_writes_ledger_snapshot_and_recovers_on_boot() {
        let runtime_home = temp_runtime_home("task-create-recover");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");

        let outcome = runtime
            .create_task(sample_create_request(agent_id.clone()))
            .expect("create");

        assert_eq!(outcome.task.status, TaskStatus::Assigned);
        assert_eq!(outcome.events.len(), 2);
        assert!(
            runtime_home
                .join("ledgers/tasks/master")
                .join(format!("{}.jsonl", outcome.task.task_id.as_str()))
                .is_file()
        );

        let recovered = TaskRuntime::boot(&runtime_home, agent_id).expect("recover");
        let task = recovered
            .query_task(&outcome.task.task_id)
            .expect("query recovered task");

        assert_eq!(task.status, TaskStatus::Assigned);
        assert_eq!(task.last_event_seq, 2);
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn create_with_no_dispatch_waits_for_agent() {
        let runtime_home = temp_runtime_home("task-waiting-agent");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");
        let mut request = sample_create_request(agent_id);
        request.dispatch = TaskDispatchRequest::None;

        let outcome = runtime.create_task(request).expect("create");

        assert_eq!(outcome.task.status, TaskStatus::WaitingAgent);
        assert!(outcome.task.assignee.is_none());
        let _ = fs::remove_dir_all(runtime_home);
    }

    #[test]
    fn boot_registers_self_agent_as_available() {
        let runtime_home = temp_runtime_home("task-self-agent");
        let agent_id = AgentId::new("master");
        let runtime = TaskRuntime::boot(&runtime_home, agent_id.clone()).expect("boot");

        let agent = runtime.query_agent(&agent_id).expect("self agent");

        assert_eq!(agent.status, AgentStatus::Available);
        assert!(agent.capabilities.contains(&"code_edit".to_owned()));
        let _ = fs::remove_dir_all(runtime_home);
    }

    fn sample_create_request(agent_id: AgentId) -> TaskCreateRequest {
        TaskCreateRequest {
            task_id: Some(TaskId::new(format!("task-{}", now_unix_seconds()))),
            title: "Implement task persistence".to_owned(),
            content: "Build task persistence skeleton".to_owned(),
            goal: "Task truth survives restart".to_owned(),
            deliverables: vec!["ledger".to_owned(), "snapshot".to_owned()],
            acceptance: vec!["recovery test passes".to_owned()],
            priority: 50,
            target_cwd: Some("/tmp/freehand-task".to_owned()),
            dispatch: TaskDispatchRequest::SelfAgent,
            parent: TaskParentRef {
                session_id: Some(SessionId::new("session-1")),
                turn_id: Some(TurnId::new("turn-1")),
                trace_id: Some(TraceId::new("trace-1")),
            },
            actor: TaskActor {
                agent_id,
                source: "control.center".to_owned(),
                session_id: Some(SessionId::new("session-1")),
                turn_id: Some(TurnId::new("turn-1")),
                trace_id: Some(TraceId::new("trace-1")),
            },
            watermark: TaskWatermark {
                metadata_id: Some("control.center:test".to_owned()),
                hook: Some("ControlHook03AfterModelResponse".to_owned()),
                action_tool_call_id: Some("toolu_task_1".to_owned()),
            },
        }
    }

    fn temp_runtime_home(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "freehand-{name}-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        ))
    }
}
