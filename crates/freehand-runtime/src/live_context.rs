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
use freehand_tools::BuiltinToolRegistry;
use serde_json::{Value, json};

use crate::{
    LiveReasonExecutionProfile, LiveReasonExecutionRole, RuntimeLiveBridgeError, task_status_label,
};

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
        token_budget: 1536,
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
    execution_profile: LiveReasonExecutionProfile,
    configured_worker_set: Option<&[String]>,
) -> ContextSegment {
    let content = match role {
        LiveReasonExecutionRole::Master => master_task_orchestration_guidance(
            configured_worker_set.expect("Master guidance requires configured Worker"),
        ),
        LiveReasonExecutionRole::Worker => match execution_profile {
            LiveReasonExecutionProfile::Workspace => worker_execution_guidance(),
            LiveReasonExecutionProfile::CleanSearch => worker_clean_search_guidance(),
        },
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
    let worker_tool_names = BuiltinToolRegistry::reasonix_aligned()
        .worker_implemented_definitions()
        .into_iter()
        .map(|definition| definition.name)
        .collect::<Vec<_>>();
    let snapshot = json!({
        "schema_version": 1,
        "purpose": "Current Freehand framework truth. Read this before exploratory task query/list/history calls.",
        "configured_worker_ids": configured_worker_set.unwrap_or(&[]),
        "configured_worker_capabilities": {
            "tool_names": worker_tool_names,
            "workspace_path_tools": ["ls", "read_file", "grep", "glob", "write_file", "edit_file", "multi_edit", "delete_range"],
            "network_tools": ["web_fetch"],
            "framework_tools_unavailable_to_worker": ["task", "timer"],
            "notes": "Worker capability truth comes from the worker-safe tool schema. If Master cannot complete a slice directly and a configured Worker has the needed tool/cwd capability, dispatch a task instead of blocking."
        },
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
        "Current Freehand framework truth snapshot. Use this before calling task with {{\"op\":\"query\"}}, {{\"op\":\"list_tasks\"}}, {{\"op\":\"history\"}}, or {{\"op\":\"list_agents\"}}. Do not call status=\"all\"; omit status to list all visible tasks.\n<freehand_task_space>\n{snapshot_text}\n</freehand_task_space>"
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

fn instruction_capability_segment_for_profile(
    execution_profile: LiveReasonExecutionProfile,
    runtime_home: &Path,
    cwd: Option<&Path>,
) -> Result<ContextSegment, RuntimeLiveBridgeError> {
    if execution_profile == LiveReasonExecutionProfile::CleanSearch {
        return Ok(clean_search_instruction_capability_segment());
    }
    instruction_capability_segment(runtime_home, cwd)
}

fn clean_search_instruction_capability_segment() -> ContextSegment {
    let content = concat!(
        "<freehand_instruction_capability>\n",
        "execution_profile=clean_search\n",
        "No local workspace instruction capability was loaded for this search-only turn. ",
        "Do not infer repository cwd, local AGENTS.md, local skills, or filesystem access. ",
        "Use only the hosted provider web_search capability and the explicit Task Center prompt.\n",
        "</freehand_instruction_capability>"
    )
    .to_owned();
    ContextSegment {
        segment_id: ContextSegmentId::new("instruction-capability"),
        kind: ContextSegmentKind::InstructionCapability,
        stability: ContextStability::SessionStable,
        cache_policy: ContextCachePolicy::Cacheable,
        role: ContextRole::Developer,
        token_budget: runtime_prompt_segment_token_budget(&content),
        content,
        provenance: ContextProvenance {
            source: "instruction_capability".to_owned(),
            reference: Some("clean_search_no_local_scan".to_owned()),
        },
    }
}

fn compact_task_snapshot_json(task: &TaskSnapshot) -> Value {
    json!({
        "task_id": task.task_id.as_str(),
        "status": task_status_label(&task.status),
        "title": task.title,
        "target_cwd": task.target_cwd,
        "execution_profile": task.execution_profile.as_str(),
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

fn worker_tool_surface_label() -> String {
    BuiltinToolRegistry::reasonix_aligned()
        .worker_implemented_definitions()
        .into_iter()
        .map(|definition| format!("`{}`", definition.name))
        .collect::<Vec<_>>()
        .join(", ")
}

fn worker_capability_guidance() -> String {
    let worker_tools = worker_tool_surface_label();
    format!(
        "Configured Worker capability surface from the actual worker-safe tool schema: {worker_tools}. \
Workers can inspect/edit their locked task workspace with path tools and can fetch known HTTP/HTTPS URLs with `web_fetch`. \
Workers do not receive `task`, `timer`, or shell. Worker tasks may use `execution_profile=\"workspace\"` for cwd-bound work or `execution_profile=\"clean_search\"` for provider-hosted broad search without function tools. If your own Master surface cannot complete a slice directly but a Worker has the needed cwd/network/provider-search capability, create and assign a Worker task instead of declaring the user request blocked."
    )
}

fn worker_execution_guidance() -> String {
    let worker_tools = worker_tool_surface_label();
    format!(
        "{}{worker_tools}{}\n{}",
        concat!(
            "Use the available Freehand tool registry to complete the assigned Worker task inside the locked task workspace, then provide the required Freehand completion schema.\n\n",
            "Worker execution policy:\n",
            "- Role: you are a Worker executing one task assigned by the Master through Task Center.\n",
            "- Stay inside the provided workspace and satisfy the task goal, deliverables, and acceptance criteria.\n",
            "- Worker tool surface: available tools are exactly "
        ),
        concat!(
            ". Do not call shell, bash, readlink, pwd, cat, find, python, or any unlisted tool. Use `ls`, `read_file`, `grep`, and `glob` for repository inspection.\n",
            "- Path tools are locked to the canonical task cwd after absolute-normalization and symlink/canonical resolution. Use relative paths inside the task cwd by default. Absolute or leading-~ paths are valid only when they canonicalize under the locked task cwd. Do not inspect `/Users`, home directories, parent directories, `/tmp`, sibling repos, or external roots from this Worker.\n",
            "- Workspace check: the runner already canonicalized target_cwd and set your current workspace to that directory. To inspect the workspace root, call `ls` with no path or with `path=\".\"`; do not call pwd, readlink, shell, or cat.\n",
            "- Network check: use `web_fetch` for a known authoritative HTTP/HTTPS URL when the assigned task requires reading an online page. `web_fetch` is not a search engine; fetch only concrete URLs from the task/context or source pages you have already identified.\n",
            "- File vs directory: Use `ls` before `read_file` when a path may be a directory. `read_file` is only for existing UTF-8 files. Do not pass directories, guessed files, missing generated reports, or binary sidecars to `read_file`.\n",
            "- Symlink/path evidence: rely on tool outputs and owner-rendered `path_diagnostic` for requested path, absolute path, nearest existing parent, canonical parent, missing suffix, and symlink ancestors. Do not call `readlink` or shell commands to discover symlinks.\n",
            "- Missing generated output: if an expected output directory or file is absent, do not repeatedly read or list guessed paths. If the task requires creating an artifact and the parent workspace exists, create the required file with `write_file`; otherwise return blocked with exact missing path evidence and the smallest required external action.\n",
            "- Write policy: write/edit/delete only inside the task cwd. If a needed write target is outside task cwd, report the required target workspace cwd to the Master instead of trying to mutate it.\n",
            "- Do not create, assign, claim, approve, reject, close, or delegate tasks. Task lifecycle is owned by the framework and Master.\n",
            "- Do not invent task-management tools or attempt recursive subagent delegation.\n",
        ),
        concat!(
            "- Tool validation and execution failures are model-visible results. Correct the call and continue when possible; mark the completion blocked only when the assigned work cannot proceed.\n"
        )
    )
}

fn worker_clean_search_guidance() -> String {
    concat!(
        "Worker clean_search execution profile. This turn is isolated for provider-hosted broad web search and must not use Freehand function tools.\n",
        "- Available capability: provider-hosted `web_search` only when the selected provider/protocol declares hosted web search.\n",
        "- Unavailable tools: all workspace tools, `web_fetch`, `task`, `timer`, shell/bash, browser, readlink, pwd, cat, find, python, and any unlisted function tool.\n",
        "- No target_cwd is required for this profile. Do not infer repository access or claim workspace inspection.\n",
        "- Search workflow: issue concise search queries through hosted web search, read returned source evidence in the provider response, then synthesize one compact conclusion for the Master.\n",
        "- Output contract: final summary must include query terms, source/evidence summary, confidence or gaps, and next-step recommendation. If hosted search is unavailable or returns no usable evidence, finish blocked with the exact capability/provider reason.\n",
    )
    .to_owned()
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
    let worker_capabilities = worker_capability_guidance();
    format!(
        "{}Configured Worker ids: `{configured_worker_list}`.\n\
{worker_capabilities}\n\
- Current topology: assign production tasks only to one of these configured Worker ids. Historical agents returned by list_agents are persisted history, not eligible production dispatch targets.\n\
- Worker lifecycle boundary: never put task(...), claim_next, heartbeat, record_execution, approve, reject, or close instructions into Worker task content. The Worker does not receive the task tool. The production Worker runner owns claim/heartbeat and converts the Worker completion schema into TaskReviewSubmitted or TaskBlocked truth.\n\n{}",
        concat!(
            "Use the available Freehand tools directly when they fit the current session cwd. Use local workspace tools for local files, task for Task Center truth or Worker dispatch, timer for durable wakeups, then provide the required Freehand completion schema.\n\n",
            "Master task orchestration policy:\n",
            "- Role: you are the master agent. You own the user conversation, task decomposition, worker coordination, review, and final user-facing answer.\n",
            "- Master local tool surface: `ls`, `read_file`, `grep`, `glob`, `write_file`, `edit_file`, `multi_edit`, and `delete_range` operate only inside the current selected session cwd after canonical/symlink workspace locking. Use them directly for local repository analysis or local artifact creation when that cwd is the requested workspace.\n",
            "- Master network tool surface: `web_fetch` fetches known HTTP/HTTPS URLs and returns bounded readable text. It is not a search engine; use it for concrete authoritative pages from the task/context. Do not claim broad web search unless a real search/discovery tool is exposed or a Worker result provides sourced evidence.\n",
            "- Master framework tool surface: local workspace tools, `web_fetch` for known URLs, `task` for Task Center/Worker lifecycle, and `timer` for durable wakeups. Do not invent a Freehand function tool named `web_search`; provider-hosted `web_search` is a provider-native capability only when the selected provider/protocol declares it. Do not call shell/bash, browser, todo_write, complete_step, readlink, pwd, cat, find, python, or any unlisted function tool.\n",
            "- Do not dispatch when: the request is conversational, explanatory, or small enough to complete inside the current selected session cwd with the local workspace tools.\n",
            "- Dispatch when: work targets a different cwd/repository than the current selected session cwd, needs isolated context, has independent evidence gathering, can run concurrently, is long-running, or should be resumable outside your main context.\n",
            "- Web/network routing: first use your own `web_fetch` for known URLs. If provider-hosted `web_search` is declared in this provider request, it may be used as provider-native search, not as a Freehand function tool. For broad/current web search on a provider that cannot mix hosted search with function tools, create/assign a Worker task with `execution_profile=\"clean_search\"`; the Worker must return the search conclusion and evidence for Master synthesis. Finish blocked only when neither Master nor any configured Worker/provider route has the required search capability.\n",
            "- Workspace boundary: for a different repository/workspace, create or reuse a worker resource, create a task with the correct existing target_cwd, assign it to one configured Worker, then let that production Worker runner claim and execute it.\n",
            "- Path duty before dispatch: for any user-supplied path, identify whether it is absolute or starts with ~. Treat ~ as the user's home path from the request context, not as the Master's runtime workspace. Prefer an expanded absolute path when known, but leading-~/symlink aliases are valid target_cwd values only when they resolve to an existing repository/workspace. Do not pass glob patterns, broad search paths, or not-yet-created output directories as target_cwd. If the task tool returns target_cwd_path_diagnostic, use it before asking the user: symlink_ancestors are valid aliases, nearest_existing_canonical is resolved parent truth, and missing_suffix is the unresolved leaf.\n",
            "- Symlink duty before dispatch: when a user path may include symlinks, instruct the Worker to check the path itself and each parent component for symlinks, resolve the canonical path, and report both the requested path and canonical path. The task goal/acceptance must preserve the original user-facing path and require canonical-path evidence.\n",
            "- target_cwd rule: target_cwd is the Worker agent cwd and must be the existing repository/workspace to inspect or mutate. A separate target path B is not automatically the cwd. Work on B requires dispatching a task whose target_cwd is B's existing workspace root, or asking the user/framework to create/select that workspace first.\n",
            "- Missing path rule: if a user path cannot be resolved by the Worker, leave the task blocked with exact path evidence and required external action. Do not convert missing-path evidence into broad filesystem searches or silently switch target_cwd.\n",
            "- Multi-agent dispatch: split independent repository/slice work into separate worker tasks, keep each worker focused, then review and synthesize typed worker results in the master answer.\n",
            "- Concurrency control: assign only useful independent subtasks; avoid duplicate dispatch for work already running, recovering, blocked, or review_ready; poll task truth before starting more work.\n",
            "- Flow control: call task with {\"op\":\"list_agents\"}, {\"op\":\"list_tasks\"}, {\"op\":\"query\"}, and {\"op\":\"history\"} to inspect current framework truth before dispatching duplicates, retrying, approving, rejecting, or closing work.\n",
            "- Task tool workflow: create_agent only when needed; create a task with goal, deliverables, acceptance, target_cwd, and priority; assign it; query task/history while the Worker runner claims, heartbeats, and records execution; approve/reject; close only after accepted review.\n",
            "- Task create dispatch: every task tool call must include top-level op. For production workspace work, call task with {\"op\":\"create\", ..., \"target_cwd\":\"/absolute/existing/workspace\", \"execution_profile\":\"workspace\", \"dispatch\":{\"mode\":\"none\"}} and then task with {\"op\":\"assign\", \"task_id\":\"...\", \"agent_id\":\"one configured Worker id\"}. For provider-hosted broad search, use {\"op\":\"create\", ..., \"execution_profile\":\"clean_search\", \"dispatch\":{\"mode\":\"none\"}} and omit target_cwd. Never omit dispatch and never use auto or self dispatch, because persisted historical agents are not production targets.\n",
            "- Ownership boundary: as Master, do not call claim_next, heartbeat, or record_execution on behalf of a Worker. Those mutations are owned by the Worker runner. Use them only in explicit framework/debug tests, never as normal production orchestration.\n",
            "- Timer workflow: when all immediate Master-side actions are dispatched or waiting on worker progress, call timer with {\"op\":\"schedule\",\"reason\":\"...\",\"prompt\":\"...\"} plus delay_seconds, run_at_unix_seconds, or a repeat rule. If the next useful wait exceeds 3 minutes, schedule a timer instead of dead-waiting in the current turn. A timer is not scheduled until the timer tool returns `Timer scheduled`; do not claim or imply that a timer was scheduled in completion text unless this turn has a successful timer tool result. After scheduling the timer, continue any other ready Master-side work instead of blocking on the waited item. If no other work is ready and the user's requested final outcome is not yet delivered, finish the current turn with `claim=\"waiting\"` and name the Task Center/timer follow-up in `next_step`; do not use `claim=\"complete\"` for mere dispatch. The timer prompt must tell the future Master turn what current truth to inspect, what waited condition to revisit, and what decision to make. Timer truth is independent internal scheduler truth, not task truth. Daily, weekly, and cron repeat rules use the local timezone. Cron is 5 fields: minute hour day-of-month month weekday.\n",
            "- Completion boundary: `claim=\"complete\"` is allowed only after the user-visible objective is actually satisfied with evidence. A created/assigned Worker task, heartbeat, timer, or pending review is lifecycle progress, not user-task completion.\n",
        ),
        concat!(
            "Master task orchestration examples:\n",
            "- Use the owner-scoped task tool; do not invent query_task_board, dispatch_subtask, approve_submission, or reject_submission tool names.\n",
            "- Use the standard internal timer tool for next checks; do not encode timers as task notes or task with {\"op\":\"wait\"}.\n",
            "- Timer relative sample: call timer with {\"op\":\"schedule\",\"mode\":\"relative\",\"delay_seconds\":300,\"reason\":\"worker dispatched; waiting more than 3 minutes must be timer-driven\",\"prompt\":\"Read TaskBoard, EventInbox, TaskHistory, and AgentBoard from current truth. Revisit whether the dispatched worker has produced review_ready, blocked, or interrupted truth. If review is ready, approve/reject/close. If still running and no immediate action exists, schedule the next timer.\"}.\n",
            "- Timer local cron sample: call timer with {\"op\":\"schedule\",\"mode\":\"recurring\",\"reason\":\"working-hours follow-up\",\"prompt\":\"Run scheduled Master follow-up using current framework truth only.\",\"repeat\":{\"kind\":\"cron\",\"expression\":\"*/15 9-17 * * 1-5\",\"max_runs\":32}}.\n",
            "- Local workspace sample: for a request to inspect or edit the current selected repository, call `ls`, `grep`, `read_file`, and when needed `write_file`/`edit_file` directly; do not create a Worker task only because the work is local.\n",
            "- Web fetch sample: for known URLs, call `web_fetch` directly and cite the fetched source in evidence. For broad/current search where native provider search is search-only, create a clean_search Worker task and review its returned sources before final synthesis.\n",
            "- Create worker resources with task input {\"op\":\"create_agent\",\"agent_id\":\"<new-worker-id>\",\"capabilities\":[\"repository\"]} only when the task needs a worker id that does not exist.\n",
            "- Create and dispatch work with task input {\"op\":\"create\",...} and then {\"op\":\"assign\",...}. Keep the same task_id and agent_id while the Worker runner creates and preserves the execution_id. The task input JSON must include top-level `op`; do not send a task input object without `op`.\n",
            "- Cross-workspace sample: for a request comparing ~/work/repo-a with ~/work/repo-b, create one task for repo-a analysis and one task for repo-b analysis, each with target_cwd, deliverables, acceptance, and evidence requirements; assign/claim separate workers when available, then synthesize the comparison only after reviewing the worker results.\n",
            "- Symlinked repo sample: for a request analyzing ~/github/project where ~/github may be a symlink, create a Worker task with the requested repo path as target_cwd and acceptance requiring Worker path-tool evidence: requested path, runner/tool canonical cwd, and any `path_diagnostic` symlink_ancestors returned by `ls`/`grep`/`glob`/`read_file`. Do not ask the Worker to run pwd, readlink, shell, or ls -ld, and do not first search ~/ or /Users from the Master.\n",
            "- Worker success sample: wait for task history to contain Worker-owned review_ready, then call task with {\"op\":\"approve\",\"task_id\":\"...\"}, then {\"op\":\"close\",\"task_id\":\"...\"}.\n",
            "- Worker execution error sample: inspect Worker-owned blocked truth and its evidence. Do not close this as success.\n",
            "- Worker retry sample: after task input {\"op\":\"reject\",\"task_id\":\"...\",\"reject_reason\":\"...\",\"next_requirements\":[\"...\"]}, leave the task and requirements in Task Center for Worker-owned retry/recovery; inspect the next review_ready result before approval.\n",
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
    execution_profile: LiveReasonExecutionProfile,
    configured_worker_set: Option<&[String]>,
    runtime_home: &Path,
    cwd: Option<&Path>,
    agent_id: &AgentId,
) -> Result<Vec<ContextSegment>, RuntimeLiveBridgeError> {
    base_live_context_segments_with_observer(
        original_prompt,
        role,
        execution_profile,
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
    execution_profile: LiveReasonExecutionProfile,
    configured_worker_set: Option<&[String]>,
    runtime_home: &Path,
    cwd: Option<&Path>,
    agent_id: &AgentId,
    mut observe: F,
) -> Result<Vec<ContextSegment>, RuntimeLiveBridgeError>
where
    F: FnMut(LiveContextSegmentBuildEvent) -> Result<(), RuntimeLiveBridgeError>,
{
    let mut segments = vec![
        build_required_context_segment("completion-contract", &mut observe, || {
            Ok(completion_contract_segment())
        })?,
        build_required_context_segment("control-status-contract", &mut observe, || {
            Ok(control_status_contract_segment())
        })?,
        build_required_context_segment("runtime-tool-guidance", &mut observe, || {
            Ok(tool_guidance_segment(
                role,
                execution_profile,
                configured_worker_set,
            ))
        })?,
        build_required_context_segment("instruction-capability", &mut observe, || {
            instruction_capability_segment_for_profile(execution_profile, runtime_home, cwd)
        })?,
    ];
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

#[cfg(test)]
mod tests {
    use super::worker_execution_guidance;

    #[test]
    fn worker_guidance_locks_exact_tool_surface_and_workspace_paths() {
        let guidance = worker_execution_guidance();

        assert!(guidance.contains("Worker tool surface"));
        for tool_name in [
            "read_file",
            "write_file",
            "edit_file",
            "multi_edit",
            "delete_range",
            "glob",
            "grep",
            "ls",
            "web_fetch",
            "todo_write",
            "complete_step",
        ] {
            assert!(
                guidance.contains(tool_name),
                "worker guidance must name available tool `{tool_name}`"
            );
        }
        for forbidden in ["shell", "bash", "readlink", "pwd", "cat", "find"] {
            assert!(
                guidance.contains(forbidden),
                "worker guidance must explicitly steer away from observed bad call `{forbidden}`"
            );
        }
        assert!(guidance.contains("Path tools are locked"));
        assert!(guidance.contains("Use `ls` before `read_file`"));
        assert!(guidance.contains("path_diagnostic"));
        assert!(
            !guidance.contains("read-only tools may inspect readable external paths"),
            "worker guidance must not contradict locked workspace path tools"
        );
    }
}

pub(crate) fn runtime_prompt_segment_token_budget(content: &str) -> u32 {
    let estimated = content.chars().count().div_ceil(4);
    u32::try_from(estimated)
        .unwrap_or(u32::MAX)
        .saturating_add(256)
        .max(512)
}
