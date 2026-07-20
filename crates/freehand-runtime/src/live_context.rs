use std::env;
use std::path::Path;
use std::time::Instant;

use freehand_blocks::completion_schema_guidance;
use freehand_contracts::{
    AgentId, ContextCachePolicy, ContextProvenance, ContextRole, ContextSegment, ContextSegmentId,
    ContextSegmentKind, ContextStability,
};
use freehand_instructions::{
    InstructionCapabilityCompileInput, compile_instruction_capability_manifest,
    render_instruction_capability_context,
};
use freehand_task::{TaskRuntime, TaskSnapshot, TaskSpaceSnapshotQuery};
use serde_json::{Value, json};

use crate::{LiveReasonExecutionRole, RuntimeLiveBridgeError, task_status_label};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LiveContextSegmentBuildStatus {
    Started,
    Completed,
    Failed,
}

impl LiveContextSegmentBuildStatus {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct LiveContextSegmentBuildEvent {
    pub segment_id: &'static str,
    pub status: LiveContextSegmentBuildStatus,
    pub elapsed_ms: Option<u128>,
    pub included: Option<bool>,
}

fn completion_contract_segment() -> ContextSegment {
    ContextSegment {
        segment_id: ContextSegmentId::new("completion-contract"),
        kind: ContextSegmentKind::CompletionContract,
        stability: ContextStability::Stable,
        cache_policy: ContextCachePolicy::CacheAnchor,
        role: ContextRole::Developer,
        content: completion_schema_guidance().prompt,
        token_budget: 1024,
        provenance: ContextProvenance {
            source: "freehand_runtime".to_owned(),
            reference: Some("completion_schema_guidance".to_owned()),
        },
    }
}

pub(crate) fn control_status_contract_segment() -> ContextSegment {
    ContextSegment {
        segment_id: ContextSegmentId::new("control-status-contract"),
        kind: ContextSegmentKind::CompletionContract,
        stability: ContextStability::Stable,
        cache_policy: ContextCachePolicy::CacheAnchor,
        role: ContextRole::Developer,
        content: concat!(
            "Optional hidden status block. Ordinary responses must omit it and include only the required <freehand_completion> block. ",
            "Output freehand_status only when a previous Freehand schema feedback explicitly asks for it. ",
            "If you output it, the opening and closing tags must match exactly:\n",
            "<<<freehand_status>>>\n",
            "{\n",
            "  \"schema_version\": 1,\n",
            "  \"status\": {\n",
            "    \"simple_question\": true | false,\n",
            "    \"task_complete\": true | false,\n",
            "    \"evidence\": \"required when task_complete=true\",\n",
            "    \"next_step\": \"required when task_complete=false and more reasoning is needed\",\n",
            "    \"blocked\": true | false,\n",
            "    \"blocked_reason\": \"required when blocked=true\",\n",
            "    \"needs_user_involvement\": true | false,\n",
            "    \"options\": [\"required when needs_user_involvement=true\"]\n",
            "  }\n",
            "}\n",
            "<</freehand_status>>>\n",
            "Do not use <<freehand_status>>, </<freehand_status>>>, <</freehand_status>>, or any shortened tag. ",
            "Status has no side effects. Use built-in tools for task mutations."
        )
        .to_owned(),
        token_budget: 1024,
        provenance: ContextProvenance {
            source: "freehand_control".to_owned(),
            reference: Some("control_status_schema_v1".to_owned()),
        },
    }
}

fn tool_guidance_segment(
    role: LiveReasonExecutionRole,
    configured_worker_set: Option<&[String]>,
) -> ContextSegment {
    let content = match role {
        LiveReasonExecutionRole::Master => master_task_orchestration_guidance(
            configured_worker_set.expect("Master guidance requires configured Worker"),
        ),
        LiveReasonExecutionRole::Worker => worker_execution_guidance().to_owned(),
    };
    ContextSegment {
        segment_id: ContextSegmentId::new("runtime-tool-guidance"),
        kind: ContextSegmentKind::DeveloperPolicy,
        stability: ContextStability::Stable,
        cache_policy: ContextCachePolicy::CacheAnchor,
        role: ContextRole::Developer,
        token_budget: runtime_prompt_segment_token_budget(&content),
        content,
        provenance: ContextProvenance {
            source: "freehand_runtime".to_owned(),
            reference: Some("runtime_tool_guidance".to_owned()),
        },
    }
}

pub(crate) fn task_space_snapshot_segment(
    runtime_home: &Path,
    agent_id: &AgentId,
    role: LiveReasonExecutionRole,
    configured_worker_set: Option<&[String]>,
) -> Result<Option<ContextSegment>, RuntimeLiveBridgeError> {
    if role != LiveReasonExecutionRole::Master {
        return Ok(None);
    }
    let task_space = TaskRuntime::query_task_space_snapshot(
        runtime_home,
        agent_id.clone(),
        TaskSpaceSnapshotQuery::default(),
    )
    .map_err(|err| RuntimeLiveBridgeError::TaskProjectionFailed(err.to_string()))?;

    let tasks = task_space
        .tasks
        .iter()
        .map(compact_task_snapshot_json)
        .collect::<Vec<_>>();
    let agents = task_space
        .agents
        .iter()
        .map(|agent| {
            json!({
                "agent_id": agent.agent_id.as_str(),
                "state": agent.state,
                "alive": agent.alive,
                "current_task_id": agent.current_task_id.as_ref().map(|id| id.as_str()),
                "current_execution_id": agent.current_execution_id,
                "next_check_at": agent.next_check_at
            })
        })
        .collect::<Vec<_>>();
    let recent_events = task_space
        .recent_events
        .iter()
        .map(|event| {
            json!({
                "cursor": event.cursor,
                "kind": event.kind,
                "task_id": event.task_id.as_str(),
                "execution_id": event.execution_id,
                "agent_id": event.agent_id.as_ref().map(|id| id.as_str()),
                "created_at": event.created_at
            })
        })
        .collect::<Vec<_>>();
    let snapshot = json!({
        "schema_version": 1,
        "purpose": "Current Freehand framework truth. Read this before exploratory task query/list/history calls.",
        "configured_worker_ids": configured_worker_set.unwrap_or(&[]),
        "valid_task_status_filters": [
            "created",
            "waiting_agent",
            "assigned",
            "running",
            "interrupted",
            "paused",
            "blocked",
            "review_submitted",
            "approved",
            "rejected",
            "failed",
            "cancelled",
            "closed"
        ],
        "known_tasks": tasks,
        "blocked_task_ids": task_space
            .blocked
            .iter()
            .map(|task| task.task_id.as_str())
            .collect::<Vec<_>>(),
        "review_ready_task_ids": task_space
            .review_ready
            .iter()
            .map(|task| task.task_id.as_str())
            .collect::<Vec<_>>(),
        "agents": agents,
        "recent_master_visible_events": recent_events
    });
    let snapshot_text = serde_json::to_string_pretty(&snapshot)
        .map_err(|err| RuntimeLiveBridgeError::TaskProjectionFailed(err.to_string()))?;
    let content = format!(
        "Current Freehand framework truth snapshot. Use this before calling task(op=\"query\"), task(op=\"list_tasks\"), task(op=\"history\"), or task(op=\"list_agents\"). Do not call status=\"all\"; omit status to list all visible tasks.\n<freehand_task_space>\n{snapshot_text}\n</freehand_task_space>"
    );
    Ok(Some(ContextSegment {
        segment_id: ContextSegmentId::new("task-space-snapshot"),
        kind: ContextSegmentKind::TaskSpaceSnapshot,
        stability: ContextStability::TurnVolatile,
        cache_policy: ContextCachePolicy::NoCache,
        role: ContextRole::Developer,
        token_budget: runtime_prompt_segment_token_budget(&content),
        content,
        provenance: ContextProvenance {
            source: "freehand_runtime".to_owned(),
            reference: Some("task_space_snapshot".to_owned()),
        },
    }))
}

pub(crate) fn instruction_capability_segment(
    runtime_home: &Path,
    cwd: Option<&Path>,
) -> Result<ContextSegment, RuntimeLiveBridgeError> {
    let cwd = match cwd {
        Some(path) => path.to_path_buf(),
        None => env::current_dir()
            .map_err(|err| RuntimeLiveBridgeError::InstructionCapabilityFailed(err.to_string()))?,
    };
    let manifest = compile_instruction_capability_manifest(InstructionCapabilityCompileInput::new(
        runtime_home.to_path_buf(),
        cwd,
    ))
    .map_err(|err| RuntimeLiveBridgeError::InstructionCapabilityFailed(err.to_string()))?;
    let content = render_instruction_capability_context(&manifest)
        .map_err(|err| RuntimeLiveBridgeError::InstructionCapabilityFailed(err.to_string()))?;
    Ok(ContextSegment {
        segment_id: ContextSegmentId::new("instruction-capability"),
        kind: ContextSegmentKind::InstructionCapability,
        stability: ContextStability::SessionStable,
        cache_policy: ContextCachePolicy::Cacheable,
        role: ContextRole::Developer,
        token_budget: runtime_prompt_segment_token_budget(&content),
        content,
        provenance: ContextProvenance {
            source: "instruction_capability".to_owned(),
            reference: Some(manifest.manifest_fingerprint.clone()),
        },
    })
}

fn compact_task_snapshot_json(task: &TaskSnapshot) -> Value {
    json!({
        "task_id": task.task_id.as_str(),
        "status": task_status_label(&task.status),
        "title": task.title,
        "target_cwd": task.target_cwd,
        "assignee_agent_id": task.assignee.as_ref().map(|assignee| assignee.agent_id.as_str()),
        "active_execution_id": task.active_execution_id,
        "review_status": task.review.status,
        "review_decision": task.review.decision,
        "last_event_seq": task.last_event_seq,
        "last_event_id": task.last_event_id,
        "last_progress_at": task.last_progress_at,
        "parent_session_id": task.parent.session_id.as_ref().map(|id| id.as_str())
    })
}

fn worker_execution_guidance() -> &'static str {
    concat!(
        "Use the available Freehand tool registry to complete the assigned Worker task inside the locked task workspace, then provide the required Freehand completion schema.\n\n",
        "Worker execution policy:\n",
        "- Role: you are a Worker executing one task assigned by the Master through Task Center.\n",
        "- Stay inside the provided workspace for mutations and satisfy the task goal, deliverables, and acceptance criteria.\n",
        "- Use governed repository read/search/write tools when needed; shell execution is not available because write intent cannot be reliably bounded. Report concrete evidence in the final completion schema.\n",
        "- Path handling: the runner has already canonicalized target_cwd and set your current workspace to that canonical directory. When the task mentions extra paths, first check whether each path is absolute, whether it contains a leading ~, and whether any path component is a symlink before reading, writing, or reporting that a path is missing.\n",
        "- Read/query policy: read-only tools may inspect readable external paths. Do not treat successful external reads as permission to write there.\n",
        "- Write policy: write/edit/delete only inside the task cwd. If a needed write target is outside task cwd, report the required target workspace cwd to the Master instead of trying to mutate it.\n",
        "- Symlink handling: if a task path is or passes through a symlink, report both the user-facing path and the canonical resolved path in evidence. Do not treat a symlinked path as missing merely because the textual path differs from the canonical path.\n",
        "- Missing-path handling: if a required source path or output path cannot be resolved from inside the locked workspace/tool policy, return blocked with the exact path, canonicalization error, and the smallest required external action. Do not invent alternate output directories.\n",
        "- Do not create, assign, claim, approve, reject, close, or delegate tasks. Task lifecycle is owned by the framework and Master.\n",
        "- Do not invent task-management tools or attempt recursive subagent delegation.\n",
        "- Tool validation and execution failures are model-visible results. Correct the call and continue when possible; mark the completion blocked only when the assigned work cannot proceed.\n"
    )
}

pub(crate) fn configured_worker_label(configured_worker_set: Option<&[String]>) -> String {
    let Some(configured_worker_set) = configured_worker_set else {
        return "<configured-worker>".to_owned();
    };
    if configured_worker_set.is_empty() {
        return "<configured-worker>".to_owned();
    }
    configured_worker_set.join("`, `")
}

fn master_task_orchestration_guidance(configured_worker_set: &[String]) -> String {
    let configured_worker_list = configured_worker_label(Some(configured_worker_set));
    format!(
        "{}Configured Worker ids: `{configured_worker_list}`.\n\
- Current topology: assign production tasks only to one of these configured Worker ids. Historical agents returned by list_agents are persisted history, not eligible production dispatch targets.\n\
- Worker lifecycle boundary: never put task(...), claim_next, heartbeat, record_execution, approve, reject, or close instructions into Worker task content. The Worker does not receive the task tool. The production Worker runner owns claim/heartbeat and converts the Worker completion schema into TaskReviewSubmitted or TaskBlocked truth.\n\n{}",
        concat!(
            "Use the available Freehand framework tools when they help orchestration. Choose task for Task Center truth or dispatch, timer for durable wakeups, then provide the required Freehand completion schema.\n\n",
            "Master task orchestration policy:\n",
            "- Role: you are the master agent. You own the user conversation, task decomposition, worker coordination, review, and final user-facing answer.\n",
            "- Dispatch when: work targets another cwd/repository, needs isolated context, has independent evidence gathering, can run concurrently, is long-running, or should be resumable outside your main context.\n",
            "- Do not dispatch when: the request is conversational, explanatory, or small enough to complete inside your current allowed workspace without isolated execution.\n",
            "- Master tool surface: your live provider tools are framework-only: task and timer. Do not try read_file, ls, grep, glob, write_file, edit_file, multi_edit, complete_step, todo_write, or shell from the Master.\n",
            "- Workspace boundary: external repository analysis, report generation, deep inspection, or writing must be delegated. Create or reuse a worker resource, create a task with the correct existing target_cwd, assign it to one configured Worker, then let that production Worker runner claim and execute it.\n",
            "- Path duty before dispatch: for any user-supplied path, identify whether it is absolute or starts with ~. Treat ~ as the user's home path from the request context, not as the Master's runtime workspace. Prefer an expanded absolute path when known, but leading-~/symlink aliases are valid target_cwd values only when they resolve to an existing repository/workspace. Do not pass glob patterns, broad search paths, or not-yet-created output directories as target_cwd. If the task tool returns target_cwd_path_diagnostic, use it before asking the user: symlink_ancestors are valid aliases, nearest_existing_canonical is resolved parent truth, and missing_suffix is the unresolved leaf. If the path is outside your allowed workspace or requires repository inspection/mutation, dispatch a Worker task whose target_cwd is the workspace to inspect or mutate.\n",
            "- Symlink duty before dispatch: when a user path may include symlinks, instruct the Worker to check the path itself and each parent component for symlinks, resolve the canonical path, and report both the requested path and canonical path. The task goal/acceptance must preserve the original user-facing path and require canonical-path evidence.\n",
            "- target_cwd rule: target_cwd is the Worker agent cwd and must be the existing repository/workspace to inspect or mutate. A separate target path B is not automatically the cwd. Work on B requires dispatching a task whose target_cwd is B's existing workspace root, or asking the user/framework to create/select that workspace first.\n",
            "- Missing path rule: if a user path cannot be resolved by the Worker, leave the task blocked with exact path evidence and required external action. Do not convert missing-path evidence into broad filesystem searches or silently switch target_cwd.\n",
            "- Multi-agent dispatch: split independent repository/slice work into separate worker tasks, keep each worker focused, then review and synthesize typed worker results in the master answer.\n",
            "- Concurrency control: assign only useful independent subtasks; avoid duplicate dispatch for work already running, recovering, blocked, or review_ready; poll task truth before starting more work.\n",
            "- Flow control: use task(op=\"list_agents\"), task(op=\"list_tasks\"), task(op=\"query\"), and task(op=\"history\") to inspect current framework truth before dispatching duplicates, retrying, approving, rejecting, or closing work.\n",
            "- Task tool workflow: create_agent only when needed; create a task with goal, deliverables, acceptance, target_cwd, and priority; assign it; query task/history while the Worker runner claims, heartbeats, and records execution; approve/reject; close only after accepted review.\n",
            "- Task create dispatch: every task tool call must include top-level op. For production worker work, call task with {\"op\":\"create\", ..., \"target_cwd\":\"/absolute/existing/workspace\", \"dispatch\":{\"mode\":\"none\"}} and then task with {\"op\":\"assign\", \"task_id\":\"...\", \"agent_id\":\"one configured Worker id\"}. Never omit dispatch and never use auto or self dispatch, because persisted historical agents are not production targets.\n",
            "- Ownership boundary: as Master, do not call claim_next, heartbeat, or record_execution on behalf of a Worker. Those mutations are owned by the Worker runner. Use them only in explicit framework/debug tests, never as normal production orchestration.\n",
            "- Timer workflow: when all immediate Master-side actions are dispatched or waiting on worker progress, call timer(op=\"schedule\") with reason, prompt, and either delay_seconds, run_at_unix_seconds, or a repeat rule. If the next useful wait exceeds 3 minutes, schedule a timer instead of dead-waiting in the current turn. A timer is not scheduled until the timer tool returns `Timer scheduled`; do not claim or imply that a timer was scheduled in completion text unless this turn has a successful timer tool result. After scheduling the timer, continue any other ready Master-side work instead of blocking on the waited item. If no other work is ready and the user's requested final outcome is not yet delivered, finish the current turn with `claim=\"waiting\"` and name the Task Center/timer follow-up in `next_step`; do not use `claim=\"complete\"` for mere dispatch. The timer prompt must tell the future Master turn what current truth to inspect, what waited condition to revisit, and what decision to make. Timer truth is independent internal scheduler truth, not task truth. Daily, weekly, and cron repeat rules use the local timezone. Cron is 5 fields: minute hour day-of-month month weekday.\n",
            "- Completion boundary: `claim=\"complete\"` is allowed only after the user-visible objective is actually satisfied with evidence. A created/assigned Worker task, heartbeat, timer, or pending review is lifecycle progress, not user-task completion.\n",
        ),
        concat!(
            "Master task orchestration examples:\n",
            "- Use the owner-scoped task tool; do not invent query_task_board, dispatch_subtask, approve_submission, or reject_submission tool names.\n",
            "- Use the standard internal timer tool for next checks; do not encode timers as task notes or task(op=\"wait\").\n",
            "- Timer relative sample: timer(op=\"schedule\", mode=\"relative\", delay_seconds=300, reason=\"worker dispatched; waiting more than 3 minutes must be timer-driven\", prompt=\"Read TaskBoard, EventInbox, TaskHistory, and AgentBoard from current truth. Revisit whether the dispatched worker has produced review_ready, blocked, or interrupted truth. If review is ready, approve/reject/close. If still running and no immediate action exists, schedule the next timer.\").\n",
            "- Timer local cron sample: timer(op=\"schedule\", mode=\"recurring\", reason=\"working-hours follow-up\", prompt=\"Run scheduled Master follow-up using current framework truth only.\", repeat={\"kind\":\"cron\",\"expression\":\"*/15 9-17 * * 1-5\",\"max_runs\":32}).\n",
            "- Create worker resources with task(op=\"create_agent\") only when the task needs a worker id that does not exist.\n",
            "- Create and dispatch work with task(op=\"create\") and task(op=\"assign\"). Keep the same task_id and agent_id while the Worker runner creates and preserves the execution_id. The task input JSON must include top-level `op`; do not send a task input object without `op`.\n",
            "- Cross-workspace sample: for a request comparing ~/work/repo-a with ~/work/repo-b, create one task for repo-a analysis and one task for repo-b analysis, each with target_cwd, deliverables, acceptance, and evidence requirements; assign/claim separate workers when available, then synthesize the comparison only after reviewing the worker results.\n",
            "- Symlinked repo sample: for a request analyzing ~/github/project where ~/github may be a symlink, create a Worker task with the requested repo path as target_cwd and acceptance requiring `pwd -P`, `ls -ld` on the path and parents, and evidence of the canonical resolved path before repository analysis. Do not first search ~/ or /Users from the Master.\n",
            "- Worker success sample: wait for task history to contain Worker-owned review_ready, then task(op=\"approve\"), then task(op=\"close\").\n",
            "- Worker execution error sample: inspect Worker-owned blocked truth and its evidence. Do not close this as success.\n",
            "- Worker retry sample: after task(op=\"reject\"), leave the task and requirements in Task Center for Worker-owned retry/recovery; inspect the next review_ready result before approval.\n",
            "- Tool validation, task transition errors, and worker execution errors are normal model-visible tool results. Use the returned result to decide the next task action instead of treating it as provider failure.\n"
        )
    )
}

pub(crate) fn original_task_segment(prompt: &str) -> ContextSegment {
    let content = format!("Original operator task:\n{prompt}");
    ContextSegment {
        segment_id: ContextSegmentId::new("original-task"),
        kind: ContextSegmentKind::TaskContract,
        stability: ContextStability::SessionStable,
        cache_policy: ContextCachePolicy::Cacheable,
        role: ContextRole::Developer,
        token_budget: runtime_prompt_segment_token_budget(&content),
        content,
        provenance: ContextProvenance {
            source: "freehand_runtime".to_owned(),
            reference: Some("original_task".to_owned()),
        },
    }
}

pub(crate) fn base_live_context_segments(
    original_prompt: &str,
    role: LiveReasonExecutionRole,
    configured_worker_set: Option<&[String]>,
    runtime_home: &Path,
    cwd: Option<&Path>,
    agent_id: &AgentId,
) -> Result<Vec<ContextSegment>, RuntimeLiveBridgeError> {
    base_live_context_segments_with_observer(
        original_prompt,
        role,
        configured_worker_set,
        runtime_home,
        cwd,
        agent_id,
        |_| Ok(()),
    )
}

pub(crate) fn base_live_context_segments_with_observer<F>(
    original_prompt: &str,
    role: LiveReasonExecutionRole,
    configured_worker_set: Option<&[String]>,
    runtime_home: &Path,
    cwd: Option<&Path>,
    agent_id: &AgentId,
    mut observe: F,
) -> Result<Vec<ContextSegment>, RuntimeLiveBridgeError>
where
    F: FnMut(LiveContextSegmentBuildEvent) -> Result<(), RuntimeLiveBridgeError>,
{
    let mut segments = Vec::new();
    segments.push(build_required_context_segment(
        "completion-contract",
        &mut observe,
        || Ok(completion_contract_segment()),
    )?);
    segments.push(build_required_context_segment(
        "control-status-contract",
        &mut observe,
        || Ok(control_status_contract_segment()),
    )?);
    segments.push(build_required_context_segment(
        "runtime-tool-guidance",
        &mut observe,
        || Ok(tool_guidance_segment(role, configured_worker_set)),
    )?);
    segments.push(build_required_context_segment(
        "instruction-capability",
        &mut observe,
        || instruction_capability_segment(runtime_home, cwd),
    )?);
    if let Some(segment) =
        build_optional_context_segment("task-space-snapshot", &mut observe, || {
            task_space_snapshot_segment(runtime_home, agent_id, role, configured_worker_set)
        })?
    {
        segments.push(segment);
    }
    segments.push(build_required_context_segment(
        "original-task",
        &mut observe,
        || Ok(original_task_segment(original_prompt)),
    )?);
    Ok(segments)
}

fn build_required_context_segment<F>(
    segment_id: &'static str,
    observe: &mut F,
    build: impl FnOnce() -> Result<ContextSegment, RuntimeLiveBridgeError>,
) -> Result<ContextSegment, RuntimeLiveBridgeError>
where
    F: FnMut(LiveContextSegmentBuildEvent) -> Result<(), RuntimeLiveBridgeError>,
{
    let started_at = Instant::now();
    observe(LiveContextSegmentBuildEvent {
        segment_id,
        status: LiveContextSegmentBuildStatus::Started,
        elapsed_ms: None,
        included: None,
    })?;
    match build() {
        Ok(segment) => {
            observe(LiveContextSegmentBuildEvent {
                segment_id,
                status: LiveContextSegmentBuildStatus::Completed,
                elapsed_ms: Some(started_at.elapsed().as_millis()),
                included: Some(true),
            })?;
            Ok(segment)
        }
        Err(error) => {
            observe(LiveContextSegmentBuildEvent {
                segment_id,
                status: LiveContextSegmentBuildStatus::Failed,
                elapsed_ms: Some(started_at.elapsed().as_millis()),
                included: Some(false),
            })?;
            Err(error)
        }
    }
}

fn build_optional_context_segment<F>(
    segment_id: &'static str,
    observe: &mut F,
    build: impl FnOnce() -> Result<Option<ContextSegment>, RuntimeLiveBridgeError>,
) -> Result<Option<ContextSegment>, RuntimeLiveBridgeError>
where
    F: FnMut(LiveContextSegmentBuildEvent) -> Result<(), RuntimeLiveBridgeError>,
{
    let started_at = Instant::now();
    observe(LiveContextSegmentBuildEvent {
        segment_id,
        status: LiveContextSegmentBuildStatus::Started,
        elapsed_ms: None,
        included: None,
    })?;
    match build() {
        Ok(segment) => {
            observe(LiveContextSegmentBuildEvent {
                segment_id,
                status: LiveContextSegmentBuildStatus::Completed,
                elapsed_ms: Some(started_at.elapsed().as_millis()),
                included: Some(segment.is_some()),
            })?;
            Ok(segment)
        }
        Err(error) => {
            observe(LiveContextSegmentBuildEvent {
                segment_id,
                status: LiveContextSegmentBuildStatus::Failed,
                elapsed_ms: Some(started_at.elapsed().as_millis()),
                included: Some(false),
            })?;
            Err(error)
        }
    }
}

pub(crate) fn runtime_prompt_segment_token_budget(content: &str) -> u32 {
    let estimated = content.chars().count().div_ceil(4);
    u32::try_from(estimated)
        .unwrap_or(u32::MAX)
        .saturating_add(256)
        .max(512)
}
