//! Tool registry and built-in tool surface for Freehand.

#![recursion_limit = "512"]

mod camo;

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use freehand_blocks::{render_tool_arguments_json, web_fetch_search_discovery};
use freehand_contracts::{
    ReasonReq04ToolCall, SearchEvidenceDelivery, ToolArgument, ToolPreviewChangeKind,
    ToolPreviewContract, ToolPreviewFileChange,
};
use freehand_provider_core::ProviderToolDefinition;
use glob::Pattern;
use regex::Regex;
use serde_json::{Value, json};
use thiserror::Error;
use walkdir::{DirEntry, WalkDir};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinToolSpec {
    pub definition: ProviderToolDefinition,
    pub read_only: bool,
    pub implemented: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolExecutionOutput {
    pub text: String,
    pub search_evidence: Option<SearchEvidenceDelivery>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuiltinToolExecutionScope {
    Framework,
    Workspace,
    Shell,
    Network,
}

impl BuiltinToolExecutionScope {
    pub fn as_str(self) -> &'static str {
        match self {
            BuiltinToolExecutionScope::Framework => "framework",
            BuiltinToolExecutionScope::Workspace => "workspace",
            BuiltinToolExecutionScope::Shell => "shell",
            BuiltinToolExecutionScope::Network => "network",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuiltinToolRegistryProjection {
    pub registry_version: String,
    pub guidance: Vec<String>,
    pub tools: Vec<BuiltinToolRegistryToolProjection>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct BuiltinToolRegistryToolProjection {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    pub read_only: bool,
    pub implemented: bool,
    pub execution_scope: String,
    pub exposed_to_master: bool,
    pub exposed_to_worker: bool,
    pub examples: Vec<String>,
    pub guidance: Vec<String>,
}

thread_local! {
    static TOOL_WORKSPACE_ROOT: RefCell<Option<PathBuf>> = const { RefCell::new(None) };
}

pub fn with_workspace_root<R>(
    root: impl AsRef<Path>,
    run: impl FnOnce() -> R,
) -> Result<R, ToolRegistryError> {
    let canonical =
        fs::canonicalize(root.as_ref()).map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: "workspace".to_owned(),
            message: format!(
                "cannot canonicalize selected workspace `{}`: {err}",
                root.as_ref().display()
            ),
        })?;
    TOOL_WORKSPACE_ROOT.with(|slot| {
        let previous = slot.replace(Some(canonical));
        let result = run();
        slot.replace(previous);
        Ok(result)
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileMutationPlan {
    path: PathBuf,
    preview_change: ToolPreviewFileChange,
    success_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MultiEditStep {
    old_string: String,
    new_string: String,
    replace_all: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathSymlinkDiagnostic {
    path: PathBuf,
    target: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PathResolutionDiagnostic {
    requested: String,
    locked_workspace: PathBuf,
    absolute: PathBuf,
    exists: bool,
    is_dir: Option<bool>,
    canonical: Option<PathBuf>,
    nearest_existing: Option<PathBuf>,
    nearest_existing_canonical: Option<PathBuf>,
    missing_suffix: Option<PathBuf>,
    symlink_ancestors: Vec<PathSymlinkDiagnostic>,
}

impl PathResolutionDiagnostic {
    fn inspect(root: &Path, requested: &str) -> Self {
        let absolute = absolutize_tool_path(root, requested);
        let metadata = fs::metadata(&absolute).ok();
        let exists = fs::symlink_metadata(&absolute).is_ok();
        let nearest_existing = nearest_existing_path(&absolute);
        let nearest_existing_canonical = nearest_existing
            .as_ref()
            .and_then(|path| fs::canonicalize(path).ok());
        let missing_suffix = nearest_existing
            .as_ref()
            .and_then(|path| absolute.strip_prefix(path).ok())
            .filter(|suffix| !suffix.as_os_str().is_empty())
            .map(Path::to_path_buf);
        Self {
            requested: requested.to_owned(),
            locked_workspace: root.to_path_buf(),
            canonical: fs::canonicalize(&absolute).ok(),
            symlink_ancestors: symlink_ancestors(&absolute),
            absolute,
            exists,
            is_dir: metadata.map(|metadata| metadata.is_dir()),
            nearest_existing,
            nearest_existing_canonical,
            missing_suffix,
        }
    }

    fn render(&self, field: &str) -> String {
        let mut fields = vec![
            format!("field={field}"),
            format!("requested=`{}`", self.requested),
            format!("locked_workspace=`{}`", self.locked_workspace.display()),
            format!("absolute=`{}`", self.absolute.display()),
            format!("exists={}", self.exists),
        ];
        if let Some(is_dir) = self.is_dir {
            fields.push(format!("is_dir={is_dir}"));
        }
        if let Some(canonical) = &self.canonical {
            fields.push(format!("canonical=`{}`", canonical.display()));
        }
        if let Some(nearest_existing) = &self.nearest_existing {
            fields.push(format!("nearest_existing=`{}`", nearest_existing.display()));
        }
        if let Some(nearest_existing_canonical) = &self.nearest_existing_canonical {
            fields.push(format!(
                "nearest_existing_canonical=`{}`",
                nearest_existing_canonical.display()
            ));
        }
        if let Some(missing_suffix) = &self.missing_suffix {
            fields.push(format!("missing_suffix=`{}`", missing_suffix.display()));
        }
        let symlinks = if self.symlink_ancestors.is_empty() {
            "[]".to_owned()
        } else {
            self.symlink_ancestors
                .iter()
                .map(|entry| format!("`{}` -> `{}`", entry.path.display(), entry.target.display()))
                .collect::<Vec<_>>()
                .join(", ")
        };
        fields.push(format!("symlink_ancestors=[{symlinks}]"));
        format!("path_diagnostic {}", fields.join(" "))
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ToolRegistryError {
    #[error("unknown tool `{0}`")]
    UnknownTool(String),
    #[error("tool `{0}` is registered but not implemented yet")]
    UnimplementedTool(String),
    #[error("tool `{tool}` arguments invalid: {message}")]
    InvalidArguments { tool: String, message: String },
    #[error("tool `{tool}` execution failed: {message}")]
    ExecutionFailed { tool: String, message: String },
    #[error("tool `{tool}` field `{field}` targets `{target}`, outside locked workspace `{root}`")]
    WorkspaceBoundaryViolation {
        tool: String,
        field: String,
        root: String,
        target: String,
    },
}

const READ_FILE_DEFAULT_LIMIT: usize = 2_000;
const BASH_DEFAULT_TIMEOUT_SECONDS: usize = 900;
const BASH_POLL_INTERVAL_MILLIS: u64 = 20;
const GLOB_MAX_RESULTS: usize = 1_000;
const GREP_MAX_MATCHES: usize = 200;
const WEB_FETCH_DEFAULT_TIMEOUT_SECONDS: usize = 20;
const WEB_FETCH_MAX_BYTES: usize = 64_000;

#[derive(Debug, Clone)]
pub struct BuiltinToolRegistry {
    tools: BTreeMap<String, BuiltinToolSpec>,
}

impl BuiltinToolRegistry {
    pub fn reasonix_aligned() -> Self {
        let mut registry = Self {
            tools: BTreeMap::new(),
        };
        for spec in reasonix_aligned_builtin_specs() {
            registry.register(spec);
        }
        registry
    }

    pub fn register(&mut self, spec: BuiltinToolSpec) {
        self.tools.insert(spec.definition.name.clone(), spec);
    }

    pub fn definitions(&self) -> Vec<ProviderToolDefinition> {
        self.tools
            .values()
            .map(|spec| spec.definition.clone())
            .collect()
    }

    pub fn implemented_definitions(&self) -> Vec<ProviderToolDefinition> {
        self.tools
            .values()
            .filter(|spec| spec.implemented)
            .map(|spec| spec.definition.clone())
            .collect()
    }

    pub fn master_implemented_definitions(&self) -> Vec<ProviderToolDefinition> {
        self.tools
            .values()
            .filter(|spec| {
                spec.implemented
                    && (matches!(spec.definition.name.as_str(), "task" | "timer" | "camo")
                        || self.execution_scope(&spec.definition.name)
                            == Some(BuiltinToolExecutionScope::Workspace)
                        || self.execution_scope(&spec.definition.name)
                            == Some(BuiltinToolExecutionScope::Network))
            })
            .map(|spec| spec.definition.clone())
            .collect()
    }

    pub fn worker_implemented_definitions(&self) -> Vec<ProviderToolDefinition> {
        self.tools
            .values()
            .filter(|spec| {
                spec.implemented
                    && spec.definition.name != "task"
                    && spec.definition.name != "timer"
                    && self.execution_scope(&spec.definition.name)
                        != Some(BuiltinToolExecutionScope::Shell)
            })
            .map(|spec| spec.definition.clone())
            .collect()
    }

    pub fn implemented_schema_fingerprint(&self) -> String {
        self.implemented_definitions()
            .iter()
            .map(canonicalize_tool_definition)
            .collect::<Vec<_>>()
            .join("\n--\n")
    }

    pub fn master_implemented_schema_fingerprint(&self) -> String {
        self.master_implemented_definitions()
            .iter()
            .map(canonicalize_tool_definition)
            .collect::<Vec<_>>()
            .join("\n--\n")
    }

    pub fn worker_implemented_schema_fingerprint(&self) -> String {
        self.worker_implemented_definitions()
            .iter()
            .map(canonicalize_tool_definition)
            .collect::<Vec<_>>()
            .join("\n--\n")
    }

    pub fn execution_scope(&self, name: &str) -> Option<BuiltinToolExecutionScope> {
        self.tools.get(name)?;
        Some(match name {
            "task" | "timer" | "todo_write" | "complete_step" => {
                BuiltinToolExecutionScope::Framework
            }
            "camo" => BuiltinToolExecutionScope::Framework,
            "bash" | "bg_jobs" | "kill_shell" | "wait_job" => BuiltinToolExecutionScope::Shell,
            "web_fetch" => BuiltinToolExecutionScope::Network,
            _ => BuiltinToolExecutionScope::Workspace,
        })
    }

    pub fn read_only(&self, name: &str) -> Option<bool> {
        self.tools.get(name).map(|spec| spec.read_only)
    }

    pub fn registry_projection(&self) -> BuiltinToolRegistryProjection {
        let master_names = self
            .master_implemented_definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();
        let worker_names = self
            .worker_implemented_definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();
        BuiltinToolRegistryProjection {
            registry_version: "reasonix-aligned-v1".to_owned(),
            guidance: vec![
                "Use the exact JSON schema shown here; do not discover tool shapes by trial calls."
                    .to_owned(),
                "Path tools are locked to the current workspace: relative paths are resolved there, leading-~ is expanded, and absolute or symlink paths are accepted only when canonicalized inside the locked workspace.".to_owned(),
                "Provider-hosted broad search is not a Freehand local function tool named web_search; inspect provider capability status or use task clean_search when configured.".to_owned(),
                "Master exposes local workspace tools, concrete-url web_fetch, task, timer, and camo; Worker exposes workspace tools, todo_write, complete_step, web_fetch, and camo; provider-hosted web_search is additionally exposed on Worker Workspace when the selected provider supports function-tool mixing.".to_owned(),
            ],
            tools: self
                .tools
                .values()
                .map(|spec| {
                    let name = spec.definition.name.clone();
                    let scope = self
                        .execution_scope(&name)
                        .map(BuiltinToolExecutionScope::as_str)
                        .unwrap_or("unknown")
                        .to_owned();
                    BuiltinToolRegistryToolProjection {
                        examples: builtin_tool_examples(&name),
                        guidance: builtin_tool_guidance(&name),
                        read_only: spec.read_only,
                        implemented: spec.implemented,
                        exposed_to_master: master_names.iter().any(|item| item == &name),
                        exposed_to_worker: worker_names.iter().any(|item| item == &name),
                        execution_scope: scope,
                        description: spec.definition.description.clone(),
                        input_schema: spec.definition.input_schema.clone(),
                        name,
                    }
                })
                .collect(),
        }
    }

    pub fn execute(
        &self,
        call: &ReasonReq04ToolCall,
    ) -> Result<ToolExecutionOutput, ToolRegistryError> {
        let name = call.tool_call.tool_name.as_str();
        let spec = self
            .tools
            .get(name)
            .ok_or_else(|| ToolRegistryError::UnknownTool(name.to_owned()))?;
        if !spec.implemented {
            return Err(ToolRegistryError::UnimplementedTool(name.to_owned()));
        }
        match name {
            "bash" => execute_bash(&call.tool_call.arguments),
            "read_file" => execute_read_file(&call.tool_call.arguments),
            "write_file" => execute_write_file(&call.tool_call.arguments),
            "edit_file" => execute_edit_file(&call.tool_call.arguments),
            "multi_edit" => execute_multi_edit(&call.tool_call.arguments),
            "glob" => execute_glob(&call.tool_call.arguments),
            "grep" => execute_grep(&call.tool_call.arguments),
            "ls" => execute_ls(&call.tool_call.arguments),
            "web_fetch" => execute_web_fetch(&call.tool_call.arguments),
            "camo" => camo::execute_camo_impl(&call.tool_call.arguments),
            "todo_write" => execute_todo_write(&call.tool_call.arguments),
            "complete_step" => execute_complete_step(&call.tool_call.arguments),
            "delete_range" => execute_delete_range(&call.tool_call.arguments),
            "task" => Err(ToolRegistryError::ExecutionFailed {
                tool: "task".to_owned(),
                message: "task execution requires the runtime task orchestrator".to_owned(),
            }),
            "timer" => Err(ToolRegistryError::ExecutionFailed {
                tool: "timer".to_owned(),
                message: "timer execution requires the runtime timer scheduler".to_owned(),
            }),
            _ => Err(ToolRegistryError::UnimplementedTool(name.to_owned())),
        }
    }

    pub fn preview(
        &self,
        call: &ReasonReq04ToolCall,
    ) -> Result<ToolPreviewContract, ToolRegistryError> {
        let name = call.tool_call.tool_name.as_str();
        let spec = self
            .tools
            .get(name)
            .ok_or_else(|| ToolRegistryError::UnknownTool(name.to_owned()))?;
        if !spec.implemented {
            return Err(ToolRegistryError::UnimplementedTool(name.to_owned()));
        }
        let change = match name {
            "write_file" => plan_write_file(&call.tool_call.arguments)?.preview_change,
            "edit_file" => plan_edit_file(&call.tool_call.arguments)?.preview_change,
            "multi_edit" => plan_multi_edit(&call.tool_call.arguments)?.preview_change,
            "delete_range" => plan_delete_range(&call.tool_call.arguments)?.preview_change,
            _ => {
                return Err(ToolRegistryError::InvalidArguments {
                    tool: name.to_owned(),
                    message: "preview is only supported for writable file-mutation tools"
                        .to_owned(),
                });
            }
        };
        Ok(ToolPreviewContract {
            tool_call_id: call.tool_call.tool_call_id.clone(),
            changes: vec![change],
        })
    }
}

fn builtin_tool_examples(name: &str) -> Vec<String> {
    match name {
        "ls" => vec![
            r#"{"path":"src","recursive":false}"#.to_owned(),
            r#"{"path":"/absolute/or/symlink/workspace/path","recursive":true}"#.to_owned(),
        ],
        "read_file" => vec![r#"{"path":"README.md","offset":0,"limit":2000}"#.to_owned()],
        "glob" => vec![
            r#"{"pattern":"src/**/*.rs"}"#.to_owned(),
            r#"{"pattern":"/absolute/or/symlink/workspace/**/*.rs"}"#.to_owned(),
        ],
        "grep" => vec![r#"{"pattern":"QueryToolRegistry","path":"crates"}"#.to_owned()],
        "write_file" => vec![r##"{"path":"output/report.md","content":"# Report\n"}"##.to_owned()],
        "edit_file" => vec![
            r#"{"path":"src/lib.rs","old_string":"old exact text","new_string":"new exact text"}"#
                .to_owned(),
        ],
        "multi_edit" => vec![
            r#"{"path":"src/lib.rs","edits":[{"old_string":"one","new_string":"two","replace_all":false}]}"#
                .to_owned(),
        ],
        "delete_range" => vec![
            r#"{"path":"src/lib.rs","start_anchor":"begin","end_anchor":"end","inclusive":true}"#
                .to_owned(),
        ],
        "web_fetch" => vec![
            r#"{"url":"https://example.com/page","timeout_seconds":20,"limit":12000}"#
                .to_owned(),
        ],
        "task" => vec![
            r#"{"op":"create","title":"Analyze module","content":"...","goal":"...","deliverables":["..."],"acceptance":["..."],"target_cwd":"/absolute/existing/repo","execution_profile":"workspace","dispatch":{"mode":"none"}}"#.to_owned(),
            r#"{"op":"assign","task_id":"task-123","agent_id":"worker-1"}"#.to_owned(),
        ],
        "timer" => vec![
            r#"{"op":"schedule","mode":"relative","delay_seconds":300,"reason":"Recheck worker result","prompt":"Read current TaskBoard/EventInbox/TaskHistory/AgentBoard truth and decide approve/reject/retry or schedule the next timer."}"#.to_owned(),
            r#"{"op":"schedule","mode":"recurring","reason":"Working-day follow-up","prompt":"Use current truth only.","repeat":{"kind":"cron","expression":"*/15 9-17 * * 1-5","max_runs":32}}"#.to_owned(),
        ],
        "todo_write" => vec![
            r#"{"todos":[{"content":"Verify output","status":"in_progress","activeForm":"Verifying output","level":0}]}"#
                .to_owned(),
        ],
        "complete_step" => vec![
            r#"{"step":"Verify","result":"passed","evidence":[{"kind":"verification","summary":"cargo test passed","command":"cargo test -p pkg"}]}"#
                .to_owned(),
        ],
        "bash" => vec![r#"{"command":"cargo test -p freehand-tools","timeout_seconds":900}"#.to_owned()],
        "camo" => vec![
            r#"{"command":"start","profile":"test","url":"https://example.com"}"#.to_owned(),
            r#"{"command":"goto","profile":"test","url":"https://example.com/page"}"#.to_owned(),
            r##"{"command":"click","profile":"test","selector":"#btn"}"##.to_owned(),
            r##"{"command":"type","profile":"test","selector":"#inp","text":"hello"}"##.to_owned(),
            r#"{"command":"screenshot","profile":"test"}"#.to_owned(),
            r#"{"command":"evaluate","profile":"test","script":"document.title"}"#.to_owned(),
            r#"{"command":"fetch-page","profile":"test","url":"https://example.com"}"#.to_owned(),
            r#"{"command":"get-readable","profile":"test"}"#.to_owned(),
            r#"{"command":"stop","profile":"test"}"#.to_owned(),
        ],
        _ => Vec::new(),
    }
}

fn builtin_tool_guidance(name: &str) -> Vec<String> {
    match name {
        "ls" | "read_file" | "glob" | "grep" | "write_file" | "edit_file" | "multi_edit"
        | "delete_range" => vec![
            "Relative paths are resolved against the locked workspace.".to_owned(),
            "Leading-~ and absolute paths are valid only when canonical/symlink resolution stays inside the locked workspace.".to_owned(),
            "External absolute paths and parent traversal are explicit boundary failures with path_diagnostic evidence.".to_owned(),
        ],
        "task" => vec![
            "Every task call must include top-level op.".to_owned(),
            "Use create then assign for production Worker dispatch; do not use status=\"all\".".to_owned(),
            "Prefer expanded absolute existing target_cwd, while symlink aliases are valid when they resolve inside the workspace.".to_owned(),
        ],
        "timer" => vec![
            "Use timer instead of dead-waiting when the useful wait exceeds three minutes.".to_owned(),
            "Timer is independent runtime truth, not Task Center truth.".to_owned(),
            "The persisted prompt must say what current truth to inspect and what decision to make after wakeup.".to_owned(),
        ],
        "web_fetch" => vec![
            "Fetches one concrete HTTP/HTTPS URL only.".to_owned(),
            "This is not broad search and is not provider-hosted web_search.".to_owned(),
        ],
        "bash" => vec![
            "Generic owner-test foreground shell tool; not exposed to Master or Worker live model surfaces.".to_owned(),
        ],
        "camo" => vec![
            "All operations go through `camo <cmd>` (subcommand word is the `command` field).".to_owned(),
            "`profile` is a named `--profile <id>` flag for every command; it is never positional.".to_owned(),
            "Positionals are limited to: `goto <url>`, `type <text>`, `fetch-page <url>`, `search <platform> <query>`, and `daemon <start|stop|status>`.".to_owned(),
            "click/hover/get-text/find-elements/upload/select take `--selector`; click/hover/find-elements also accept a `--text` locator.".to_owned(),
            "Create a profile with `camo profile create <id>` before first use; without an existing profile the daemon fails fast with `profile not found`.".to_owned(),
            "A camo daemon may already be running for the target profile. Before calling `start`/`daemon start`, run `camo daemon status` to check; if a daemon is already running for the profile, call `goto`, `fetch-page`, or the inspection command directly and never call `start` again. `camo start`/`camo daemon start` hang when a daemon for that profile is already active, so do not call them when the profile is already started.".to_owned(),
            "For long-running browser tasks, start the named profile once, reuse it across calls, then `camo stop` when done. Do not start/stop per call.".to_owned(),
            "JavaScript eval is `camo evaluate --script <js>` (named `--script`).".to_owned(),
            "For stable page fetch, prefer `camo fetch-page <url>` then `camo get-readable` over raw HTTP when the target needs browser rendering.".to_owned(),
        ],
        "todo_write" | "complete_step" => vec![
            "Worker-safe framework progress tool; not exposed to Master live turns.".to_owned(),
        ],
        _ => Vec::new(),
    }
}

pub fn reasonix_aligned_builtin_specs() -> Vec<BuiltinToolSpec> {
    vec![
        spec(
            "bash",
            false,
            true,
            "Run a foreground shell command from the locked workspace root.",
            json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string", "description": "Command to run"},
                    "timeout_seconds": {"type": "integer", "minimum": 1}
                },
                "required": ["command"]
            }),
        ),
        spec(
            "bg_jobs",
            true,
            false,
            "List background shell jobs.",
            json!({"type": "object", "properties": {}}),
        ),
        spec(
            "kill_shell",
            false,
            false,
            "Stop a background shell job by id.",
            json!({
                "type": "object",
                "properties": {"job_id": {"type": "string"}},
                "required": ["job_id"]
            }),
        ),
        spec(
            "wait_job",
            true,
            false,
            "Wait for a background shell job.",
            json!({
                "type": "object",
                "properties": {"job_id": {"type": "string"}},
                "required": ["job_id"]
            }),
        ),
        spec(
            "read_file",
            true,
            true,
            "Read one UTF-8 text file with optional line offset/limit. Relative paths are resolved from the locked workspace; leading `~` or absolute paths are valid only when canonical/symlink resolution stays inside that locked workspace. Use `ls` first when the path might be a directory. Do not pass directories, not-yet-created output directories or files, external absolute paths, binary sidecars, or guessed files.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Existing file path inside the locked workspace. Prefer a relative path. Leading `~` or absolute paths are accepted only if canonical/symlink resolution stays under the locked workspace. Directories must use `ls`, not `read_file`."},
                    "offset": {"type": "integer", "minimum": 0},
                    "limit": {"type": "integer", "minimum": 1}
                },
                "required": ["path"]
            }),
        ),
        spec(
            "write_file",
            false,
            true,
            "Write content to a file under the locked workspace, overwriting existing content. Prefer relative paths; absolute or leading-`~` paths are valid only when canonical/symlink resolution stays inside the locked workspace.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Target file path inside the locked workspace. Prefer relative paths; absolute or leading-~ aliases must resolve under the locked workspace."},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"]
            }),
        ),
        spec(
            "edit_file",
            false,
            true,
            "Replace an exact string in a file under the locked workspace. Prefer relative paths; absolute or leading-`~` paths are valid only when canonical/symlink resolution stays inside the locked workspace.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Existing file path inside the locked workspace. Prefer relative paths; absolute or leading-~ aliases must resolve under the locked workspace."},
                    "old_string": {"type": "string"},
                    "new_string": {"type": "string"}
                },
                "required": ["path", "old_string", "new_string"]
            }),
        ),
        spec(
            "multi_edit",
            false,
            true,
            "Apply a list of edits atomically to one file under the locked workspace. Prefer relative paths; absolute or leading-`~` paths are valid only when canonical/symlink resolution stays inside the locked workspace.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Existing file path inside the locked workspace. Prefer relative paths; absolute or leading-~ aliases must resolve under the locked workspace."},
                    "edits": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "properties": {
                                "old_string": {"type": "string"},
                                "new_string": {"type": "string"},
                                "replace_all": {"type": "boolean"}
                            },
                            "required": ["old_string", "new_string"]
                        }
                    }
                },
                "required": ["path", "edits"]
            }),
        ),
        spec(
            "delete_range",
            false,
            true,
            "Delete a contiguous text range from a file under the locked workspace using exact start/end text anchors. Prefer relative paths; absolute or leading-`~` paths are valid only when canonical/symlink resolution stays inside the locked workspace.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Existing file path inside the locked workspace. Prefer relative paths; absolute or leading-~ aliases must resolve under the locked workspace."},
                    "start_anchor": {"type": "string"},
                    "end_anchor": {"type": "string"},
                    "inclusive": {"type": "boolean"}
                },
                "required": ["path", "start_anchor", "end_anchor"]
            }),
        ),
        spec(
            "delete_symbol",
            false,
            false,
            "Delete a named Go symbol from a source file using AST parsing.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "name": {"type": "string"},
                    "kind": {"type": "string"},
                    "parent": {"type": "string"}
                },
                "required": ["path", "name"]
            }),
        ),
        spec(
            "glob",
            true,
            true,
            "Find files matching a glob pattern inside the current locked workspace. Prefer relative patterns such as `main/**/*.cc`; relative paths are resolved from the locked workspace, leading `~` is expanded, and an absolute pattern is valid only when it resolves inside the locked workspace. Do not pass parent traversal, broad external roots, or exact known file paths; use `ls` for directories/existence checks and `read_file` for known files.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": {
                        "type": "string",
                        "description": "Workspace-scoped glob. Preferred: relative patterns like \"src/**/*.rs\". Also accepted: absolute or leading-~ patterns that resolve under the locked workspace root after symlink/canonical path handling. Invalid: \"../**\" or absolute paths outside the locked workspace."
                    }
                },
                "required": ["pattern"]
            }),
        ),
        spec(
            "grep",
            true,
            true,
            "Search for a regular expression in files under the locked workspace. Omit `path` for the workspace root; relative paths are resolved from the locked workspace; absolute or leading-`~` paths are valid only when canonical/symlink resolution stays inside the locked workspace.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string", "description": "Optional existing file or directory inside the locked workspace. Prefer relative paths; absolute or leading-~ aliases must resolve under the locked workspace."}
                },
                "required": ["pattern"]
            }),
        ),
        spec(
            "ls",
            true,
            true,
            "List directory entries or report one file entry under the locked workspace. Omit `path` for the workspace root. Relative paths are resolved from the locked workspace; absolute or leading-`~` paths are valid only when canonical/symlink resolution stays inside the locked workspace. Use this before `read_file` when you are unsure whether a path is a file or directory, and to verify whether a generated output file exists. Do not keep listing guessed missing output directories; create required artifacts with `write_file` only when the parent workspace exists.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "Existing file or directory path inside the locked workspace. Omit for current workspace. Prefer relative paths; absolute or leading-~ aliases must resolve under the locked workspace."},
                    "recursive": {"type": "boolean"}
                }
            }),
        ),
        spec(
            "web_fetch",
            true,
            true,
            "Fetch one HTTP/HTTPS URL and return bounded readable text content. Use this for known authoritative URLs or pages discovered from task context. This is not a search engine; if you need broad discovery, use known source pages, indexed pages supplied by the user/task, or delegate to a Worker only when the Worker tool surface also exposes `web_fetch`.",
            json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string", "description": "Absolute http:// or https:// URL to fetch."},
                    "domain_plan_ref": {"type": "string", "description": "Sourced-search recovery only: persisted search domain plan delivery id."},
                    "timeout_seconds": {"type": "integer", "minimum": 1, "maximum": 60},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 64000}
                },
                "required": ["url"]
            }),
        ),
        spec(
            "notebook_edit",
            false,
            false,
            "Edit a notebook cell.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "cell_id": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "cell_id", "content"]
            }),
        ),
        spec(
            "todo_write",
            true,
            true,
            "Record and update a structured task list for the current work.",
            json!({
                "type": "object",
                "properties": {
                    "todos": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "content": {"type": "string"},
                                "status": {"type": "string", "enum": ["pending", "in_progress", "completed"]},
                                "activeForm": {"type": "string"},
                                "level": {"type": "integer", "enum": [0, 1]}
                            },
                            "required": ["content", "status"]
                        }
                    }
                },
                "required": ["todos"]
            }),
        ),
        spec(
            "complete_step",
            true,
            true,
            "Record evidence-backed completion of one step.",
            json!({
                "type": "object",
                "properties": {
                    "step": {"type": "string"},
                    "result": {"type": "string"},
                    "evidence": {
                        "type": "array",
                        "minItems": 1,
                        "items": {
                            "type": "object",
                            "properties": {
                                "kind": {"type": "string", "enum": ["verification", "diff", "files", "manual"]},
                                "summary": {"type": "string"},
                                "command": {"type": "string"},
                                "paths": {"type": "array", "items": {"type": "string"}}
                            },
                            "required": ["kind", "summary"]
                        }
                    },
                    "notes": {"type": "string"}
                },
                "required": ["step", "result", "evidence"]
            }),
        ),
        spec(
            "task",
            false,
            true,
            "Task Center call shape is strict: the top-level JSON object must include \"op\". For workspace work, call exactly like {\"op\":\"create\",\"title\":\"...\",\"content\":\"...\",\"goal\":\"...\",\"deliverables\":[\"...\"],\"acceptance\":[\"...\"],\"target_cwd\":\"/absolute/existing/repo\",\"execution_profile\":\"workspace\",\"dispatch\":{\"mode\":\"none\"}}; then assign with {\"op\":\"assign\",\"task_id\":\"...\",\"agent_id\":\"<configured Worker>\"}. For provider-hosted broad search, use \"execution_profile\":\"clean_search\" and omit target_cwd. Do not call task with only a title/content payload and do not omit op. Prefer the current TaskSpaceSnapshot before query/list/history; use tools only for concrete mutations or decision-critical ledger truth.",
            json!({
                "type": "object",
                "properties": {
                    "op": {
                        "type": "string",
                        "description": "Required top-level Task Center operation and first-call discriminator. Never omit op. Never call task with only title/content/goal. Master production workspace flow: {\"op\":\"create\", title/content/goal/deliverables/acceptance/target_cwd, \"execution_profile\":\"workspace\", \"dispatch\":{\"mode\":\"none\"}}, then {\"op\":\"assign\", \"task_id\":..., \"agent_id\": configured Worker}. Broad provider-hosted search flow: create with \"execution_profile\":\"clean_search\" and no target_cwd. Query/list/history are for specific missing truth only; do not use status=\"all\". Worker runner, not Master, owns claim_next/heartbeat/record_execution in production.",
                        "enum": ["create", "query", "list_tasks", "history", "append", "pause", "resume", "heartbeat", "assign", "claim_next", "record_execution", "cancel", "submit_review", "approve", "reject", "close", "list_agents", "query_agent", "create_agent", "close_agent"]
                    },
                    "task_id": {"type": "string"},
                    "execution_id": {
                        "type": "string",
                        "description": "Required for claim_next and for record_execution status updates so worker results stay paired to the same execution."
                    },
                    "status": {
                        "type": "string",
                        "description": "For list_tasks this filters one exact task status: created, waiting_agent, assigned, running, interrupted, paused, blocked, review_submitted, approved, rejected, failed, cancelled, or closed. Omit status to list all visible tasks; do not pass all. For record_execution this reports worker state: running, recovering, blocked, interrupted, or review_ready."
                    },
                    "retry_count": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Required when record_execution status is recovering after a rejected or failed attempt."
                    },
                    "ttl_seconds": {"type": "integer", "minimum": 1},
                    "note": {"type": "string"},
                    "title": {"type": "string"},
                    "content": {"type": "string"},
                    "goal": {"type": "string"},
                    "deliverables": {"type": "array", "items": {"type": "string"}},
                    "acceptance": {"type": "array", "items": {"type": "string"}},
                    "priority": {"type": "integer"},
                    "target_cwd": {
                        "type": "string",
                        "description": "Existing Worker repository/workspace cwd. Prefer an expanded absolute path such as /absolute/existing/workspace. Leading-~/symlink aliases are valid only when they resolve to an existing workspace; preserve the user-facing path in task content/acceptance and require canonical-path evidence. Do not pass glob patterns, broad search paths, or a not-yet-created output directory."
                    },
                    "execution_profile": {
                        "type": "string",
                        "description": "Worker execution profile. Use workspace for normal cwd-bound file/repo work and include target_cwd. Use clean_search only for provider-hosted broad/current web search; omit target_cwd and require the Worker to return sourced search conclusions for Master synthesis. Do not use clean_search for local files, code edits, tests, or known-URL web_fetch work.",
                        "enum": ["workspace", "clean_search"]
                    },
                    "dispatch": {
                        "type": "object",
                        "description": "For normal Master dispatch, use {\"mode\":\"none\"} and then call task with {\"op\":\"assign\",\"task_id\":\"...\",\"agent_id\":\"<configured Worker>\"}, or use {\"mode\":\"agent\",\"agent_id\":\"<configured Worker>\"}. Do not use auto/self in production.",
                        "properties": {
                            "mode": {"type": "string", "enum": ["none", "self", "agent", "auto"]},
                            "agent_id": {"type": "string"},
                            "allow_create_agent": {"type": "boolean"}
                        },
                        "required": ["mode"]
                    },
                    "summary": {"type": "string"},
                    "evidence": {"type": "array", "items": {"type": "string"}},
                    "reject_reason": {"type": "string"},
                    "phase": {"type": "string"},
                    "next_requirements": {"type": "array", "items": {"type": "string"}},
                    "agent_id": {"type": "string"},
                    "capabilities": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["op"],
                "additionalProperties": false
            }),
        ),
        spec(
            "timer",
            false,
            true,
            "Schedule internal wakeups for the Master. This is independent from task truth: use it after dispatching work when the Master should wake later, read current framework truth, and continue. If the next useful wait exceeds 3 minutes, schedule a timer instead of dead-waiting in the current turn; after scheduling, continue any other ready Master-side work. If only async Worker/timer lifecycle remains, end with completion claim `waiting`, not `complete`. Do not claim a timer was scheduled unless this timer tool returned `Timer scheduled` in the current turn. Supports relative countdowns, absolute Unix timestamps, local-time daily/weekly recurrence, and 5-field local-time cron expressions. Every schedule must include a concrete reason and the exact prompt to use when the timer fires.",
            json!({
                "type": "object",
                "properties": {
                    "op": {
                        "type": "string",
                        "description": "Timer operation. Use schedule to create/update a wakeup, cancel to stop one, and list to inspect active timers.",
                        "enum": ["schedule", "cancel", "list"]
                    },
                    "timer_id": {
                        "type": "string",
                        "description": "Optional stable id for schedule/cancel; generated when omitted for schedule."
                    },
                    "reason": {
                        "type": "string",
                        "description": "Why this wakeup is needed. Required for schedule."
                    },
                    "prompt": {
                        "type": "string",
                        "description": "Prompt used when the timer fires to continue the internal Master workflow. Must tell Master what current truth to inspect, what waited condition to revisit, and what decision to make. Must tell Master to read current framework truth and not assume prior state from memory. Required for schedule."
                    },
                    "mode": {
                        "type": "string",
                        "description": "Schedule mode for schedule op.",
                        "enum": ["relative", "absolute", "recurring"]
                    },
                    "delay_seconds": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Relative countdown in seconds."
                    },
                    "run_at_unix_seconds": {
                        "type": "integer",
                        "minimum": 0,
                        "description": "Absolute Unix timestamp in seconds."
                    },
                    "repeat": {
                        "type": "object",
                        "description": "Recurring schedule rule. interval_seconds is relative repeat; daily/weekly use local time-of-day seconds and optional local weekdays; cron uses a 5-field local-time expression: minute hour day-of-month month weekday.",
                        "properties": {
                            "kind": {"type": "string", "enum": ["interval", "daily", "weekly", "cron"]},
                            "interval_seconds": {"type": "integer", "minimum": 1},
                            "time_of_day_seconds_local": {
                                "type": "integer",
                                "minimum": 0,
                                "maximum": 86399,
                                "description": "Local-time seconds after midnight, used by daily/weekly. Example 09:30 local = 34200."
                            },
                            "cron_expression": {
                                "type": "string",
                                "description": "Deprecated alias for expression; use expression."
                            },
                            "expression": {
                                "type": "string",
                                "description": "5-field local-time cron expression: minute hour day-of-month month weekday. Supports *, comma lists, ranges, and /steps. Weekday Sunday=0 through Saturday=6."
                            },
                            "weekdays": {
                                "type": "array",
                                "items": {"type": "integer", "minimum": 0, "maximum": 6},
                                "description": "Local weekdays, Sunday=0 through Saturday=6. Workdays are [1,2,3,4,5]."
                            },
                            "skip_weekends": {
                                "type": "boolean",
                                "description": "For daily local-time schedules, skip local Saturday/Sunday."
                            },
                            "max_runs": {"type": "integer", "minimum": 1}
                        },
                        "required": ["kind"]
                    },
                    "max_runs": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Maximum number of firings. Defaults to 1 for one-shot timers."
                    }
                },
                "examples": [
                    {
                        "op": "schedule",
                        "mode": "relative",
                        "delay_seconds": 300,
                        "reason": "Worker was dispatched; waiting more than 3 minutes should be timer-driven instead of dead-waiting.",
                        "prompt": "Read TaskBoard, EventInbox, TaskHistory, and AgentBoard from current truth. Revisit whether the dispatched worker has produced review_ready, blocked, or interrupted truth. If review is ready, approve/reject/close. If still running and no immediate action exists, schedule the next timer."
                    },
                    {
                        "op": "schedule",
                        "mode": "recurring",
                        "reason": "Run a working-day local-time check.",
                        "prompt": "Run scheduled Master follow-up using current framework truth only.",
                        "repeat": {
                            "kind": "weekly",
                            "time_of_day_seconds_local": 34200,
                            "weekdays": [1, 2, 3, 4, 5],
                            "max_runs": 20
                        }
                    },
                    {
                        "op": "schedule",
                        "mode": "recurring",
                        "reason": "Cron-based local-time Master check.",
                        "prompt": "Run cron-triggered Master follow-up using current framework truth only.",
                        "repeat": {
                            "kind": "cron",
                            "expression": "*/15 9-17 * * 1-5",
                            "max_runs": 32
                        }
                    }
                ],
                "required": ["op"]
            }),
        ),
        spec(
            "camo",
            true,
            true,
            "Browser automation via @web-auto/camo@0.4.2 CLI (camo.v2 protocol/v1, camoufox). All operations go through `camo <subcommand>`. `profile` is a named `--profile <id>` flag for every command; it must exist (create with `camo profile create <id>`). Most arguments are named flags; only `goto <url>`, `type <text>`, `fetch-page <url>`, `search <platform> <query>`, and `daemon <start|stop|status>` take positionals. See `camo help` for all subcommands.",
            json!({
                "type": "object",
                "properties": {
                    "profile": {
                        "type": "string",
                        "description": "Browser profile id, passed as `--profile <id>`. Must be created with `camo profile create <id>` before use. Omit to use the default profile."
                    },
                    "command": {
                        "type": "string",
                        "description": "camo subcommand (0.4.2). Lifecycle: start|stop. Navigation: goto|screenshot. Interaction: click|type|scroll|hover|upload|select. Inspection: snapshot|get-text|get-page-info|find-elements|get-readable|get-cookies|set-cookies|set-user-agent|set-viewport|wait-dom-stable. Tabs: new-tab|close-tab|list-tabs|switch-tab. DevTools: evaluate. Wait: wait. Daemon: daemon. Search: search|scroll-and-collect. Fetch: fetch-page. Run `camo help` for full list.",
                        "enum": [
                            "start", "stop", "goto", "screenshot", "click", "type",
                            "scroll", "hover", "snapshot", "get-text", "get-page-info",
                            "find-elements", "get-readable", "get-cookies", "set-cookies",
                            "set-user-agent", "set-viewport", "wait-dom-stable", "new-tab",
                            "close-tab", "list-tabs", "switch-tab", "evaluate", "wait",
                            "upload", "select", "daemon", "search", "scroll-and-collect",
                            "fetch-page"
                        ]
                    },
                    "url": {"type": "string", "description": "URL. Positional for goto/fetch-page; named --url flag for start/new-tab."},
                    "selector": {"type": "string", "description": "CSS selector (named --selector) for click/type/hover/get-text/find-elements/upload/select."},
                    "text": {"type": "string", "description": "Text. Positional for type; named --text for click/hover/find-elements text locator."},
                    "button": {"type": "string", "description": "Mouse button for click: left|middle|right.", "enum": ["left", "middle", "right"]},
                    "waitUntil": {"type": "string", "description": "Page wait condition for goto: load|domcontentloaded|networkidle.", "enum": ["load", "domcontentloaded", "networkidle"]},
                    "script": {"type": "string", "description": "JavaScript for evaluate (named --script)."},
                    "expression": {"type": "string", "description": "Alias of script for evaluate."},
                    "for": {"type": "string", "description": "Wait condition for wait: load|domcontentloaded|networkidle|selector|text|url.", "enum": ["load", "domcontentloaded", "networkidle", "selector", "text", "url"]},
                    "target": {"type": "string", "description": "Target for wait selector/text/url condition."},
                    "timeout": {"type": "integer", "description": "Timeout in ms (wait/fetch-page/wait-dom-stable)."},
                    "poll": {"type": "integer", "description": "Poll interval ms for wait-dom-stable."},
                    "delay": {"type": "integer", "description": "Per-keystroke delay ms for type, or inter-scroll delay ms for scroll-and-collect."},
                    "x": {"type": "integer", "description": "Horizontal scroll delta px for scroll."},
                    "y": {"type": "integer", "description": "Vertical scroll delta px for scroll."},
                    "width": {"type": "integer", "description": "Viewport width px for set-viewport."},
                    "height": {"type": "integer", "description": "Viewport height px for set-viewport."},
                    "maxLength": {"type": "integer", "description": "Max readable text length for get-readable (default 50000)."},
                    "scrollCount": {"type": "integer", "description": "Number of scrolls for scroll-and-collect."},
                    "tabId": {"type": "integer", "description": "Tab index for close-tab/switch-tab."},
                    "format": {"type": "string", "description": "Snapshot output format: json|yaml.", "enum": ["json", "yaml"]},
                    "path": {"type": "string", "description": "Output file path for screenshot."},
                    "file": {"type": "string", "description": "File path to upload for upload."},
                    "value": {"type": "string", "description": "Option value for select."},
                    "cookies": {"type": "string", "description": "JSON cookies array for set-cookies, or cookie file path for search."},
                    "ua": {"type": "string", "description": "User agent string for set-user-agent."},
                    "subcommand": {"type": "string", "description": "Daemon subcommand (positional): start|stop|status.", "enum": ["start", "stop", "status"]},
                    "platform": {"type": "string", "description": "Search platform for search (positional): xhs, douyin, etc."},
                    "query": {"type": "string", "description": "Search query for search (positional)."},
                    "max-results": {"type": "integer", "description": "Max results for search."},
                    "headless": {"type": "boolean", "description": "Run browser headless for start."},
                    "ephemeral": {"type": "boolean", "description": "Daemon runs in ephemeral mode for daemon start."}
                },
                "required": ["command"]
            }),
        ),
    ]
}

fn spec(
    name: &str,
    read_only: bool,
    implemented: bool,
    description: &str,
    input_schema: Value,
) -> BuiltinToolSpec {
    BuiltinToolSpec {
        definition: ProviderToolDefinition {
            name: name.to_owned(),
            description: description.to_owned(),
            input_schema,
        },
        read_only,
        implemented,
    }
}

fn execute_bash(arguments: &[ToolArgument]) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let command = required_string(arguments, "bash", "command")?;
    let timeout_seconds = optional_usize(arguments, "bash", "timeout_seconds")?
        .unwrap_or(BASH_DEFAULT_TIMEOUT_SECONDS);
    if timeout_seconds == 0 {
        return Err(ToolRegistryError::InvalidArguments {
            tool: "bash".to_owned(),
            message: "`timeout_seconds` must be at least 1".to_owned(),
        });
    }
    let root = locked_workspace_root("bash")?;
    let output_path = temp_tool_output_path("bash")?;
    let output_file = fs::OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&output_path)
        .map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: "bash".to_owned(),
            message: format!(
                "cannot create command output capture `{}`: {err}",
                output_path.display()
            ),
        })?;
    let stderr_file =
        output_file
            .try_clone()
            .map_err(|err| ToolRegistryError::ExecutionFailed {
                tool: "bash".to_owned(),
                message: format!(
                    "cannot clone command output capture `{}`: {err}",
                    output_path.display()
                ),
            })?;

    let mut child = Command::new("bash")
        .arg("-lc")
        .arg(command)
        .current_dir(&root)
        .stdout(Stdio::from(output_file))
        .stderr(Stdio::from(stderr_file))
        .spawn()
        .map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: "bash".to_owned(),
            message: format!("cannot spawn `bash`: {err}"),
        })?;

    let timeout = Duration::from_secs(u64::try_from(timeout_seconds).map_err(|_| {
        ToolRegistryError::InvalidArguments {
            tool: "bash".to_owned(),
            message: "`timeout_seconds` is too large".to_owned(),
        }
    })?);
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                let output = read_command_output(&output_path, "bash")?;
                if status.success() {
                    return Ok(ToolExecutionOutput {
                        text: render_shell_output(output),
                        search_evidence: None,
                    });
                }
                return Err(ToolRegistryError::ExecutionFailed {
                    tool: "bash".to_owned(),
                    message: format!(
                        "command exited with status {}{}",
                        status,
                        render_shell_output_suffix(&output)
                    ),
                });
            }
            Ok(None) => {
                if start.elapsed() >= timeout {
                    child
                        .kill()
                        .map_err(|err| ToolRegistryError::ExecutionFailed {
                            tool: "bash".to_owned(),
                            message: format!("cannot kill timed-out command: {err}"),
                        })?;
                    let _ = child.wait();
                    let output = read_command_output(&output_path, "bash")?;
                    return Err(ToolRegistryError::ExecutionFailed {
                        tool: "bash".to_owned(),
                        message: format!(
                            "command timed out after {} second(s){}",
                            timeout_seconds,
                            render_shell_output_suffix(&output)
                        ),
                    });
                }
                thread::sleep(Duration::from_millis(BASH_POLL_INTERVAL_MILLIS));
            }
            Err(err) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ToolRegistryError::ExecutionFailed {
                    tool: "bash".to_owned(),
                    message: format!("cannot poll command status: {err}"),
                });
            }
        }
    }
}

fn execute_todo_write(
    arguments: &[ToolArgument],
) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let todos = argument_value(arguments, "todos")
        .and_then(Value::as_array)
        .ok_or_else(|| ToolRegistryError::InvalidArguments {
            tool: "todo_write".to_owned(),
            message: "`todos` array is required".to_owned(),
        })?;
    let mut completed = 0usize;
    let mut in_progress = 0usize;
    let mut pending = 0usize;
    for (index, todo) in todos.iter().enumerate() {
        let object = todo
            .as_object()
            .ok_or_else(|| invalid_tool_argument("todo_write", index, "todo must be an object"))?;
        let content = object
            .get("content")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if content.is_empty() {
            return Err(invalid_tool_argument(
                "todo_write",
                index,
                "`content` is required",
            ));
        }
        match object
            .get("status")
            .and_then(Value::as_str)
            .unwrap_or("pending")
        {
            "completed" => completed += 1,
            "in_progress" => in_progress += 1,
            "pending" => pending += 1,
            other => {
                return Err(invalid_tool_argument(
                    "todo_write",
                    index,
                    &format!("invalid status `{other}`"),
                ));
            }
        }
    }
    Ok(ToolExecutionOutput {
        text: format!(
            "Todos updated: {} total - {} completed, {} in progress, {} pending.",
            todos.len(),
            completed,
            in_progress,
            pending
        ),
        search_evidence: None,
    })
}

fn execute_complete_step(
    arguments: &[ToolArgument],
) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let step = required_string(arguments, "complete_step", "step")?;
    let result = required_string(arguments, "complete_step", "result")?;
    let evidence = argument_value(arguments, "evidence")
        .and_then(Value::as_array)
        .ok_or_else(|| ToolRegistryError::InvalidArguments {
            tool: "complete_step".to_owned(),
            message: "`evidence` array is required".to_owned(),
        })?;
    if evidence.is_empty() {
        return Err(ToolRegistryError::InvalidArguments {
            tool: "complete_step".to_owned(),
            message: "`evidence` must contain at least one item".to_owned(),
        });
    }
    Ok(ToolExecutionOutput {
        text: format!(
            "Step `{step}` signed off with {} evidence item(s). Result: {result}",
            evidence.len()
        ),
        search_evidence: None,
    })
}

fn execute_web_fetch(arguments: &[ToolArgument]) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let url = required_string(arguments, "web_fetch", "url")?;
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err(ToolRegistryError::InvalidArguments {
            tool: "web_fetch".to_owned(),
            message: "`url` must start with http:// or https://".to_owned(),
        });
    }
    let timeout_seconds = optional_usize(arguments, "web_fetch", "timeout_seconds")?
        .unwrap_or(WEB_FETCH_DEFAULT_TIMEOUT_SECONDS);
    if timeout_seconds == 0 || timeout_seconds > 60 {
        return Err(ToolRegistryError::InvalidArguments {
            tool: "web_fetch".to_owned(),
            message: "`timeout_seconds` must be between 1 and 60".to_owned(),
        });
    }
    let limit = optional_usize(arguments, "web_fetch", "limit")?.unwrap_or(WEB_FETCH_MAX_BYTES);
    if limit == 0 || limit > WEB_FETCH_MAX_BYTES {
        return Err(ToolRegistryError::InvalidArguments {
            tool: "web_fetch".to_owned(),
            message: format!("`limit` must be between 1 and {WEB_FETCH_MAX_BYTES}"),
        });
    }
    let client = reqwest::blocking::Client::builder()
        .timeout(Duration::from_secs(timeout_seconds as u64))
        .user_agent("freehand-web-fetch/1")
        .build()
        .map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: "web_fetch".to_owned(),
            message: format!("cannot build HTTP client: {err}"),
        })?;
    let mut response =
        client
            .get(url)
            .send()
            .map_err(|err| ToolRegistryError::ExecutionFailed {
                tool: "web_fetch".to_owned(),
                message: format!("request failed for `{url}`: {err}"),
            })?;
    let status = response.status();
    let content_type = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("unknown")
        .to_owned();
    let mut buffer = Vec::new();
    let read_limit = limit.saturating_add(1);
    response
        .by_ref()
        .take(read_limit as u64)
        .read_to_end(&mut buffer)
        .map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: "web_fetch".to_owned(),
            message: format!("cannot read response body from `{url}`: {err}"),
        })?;
    let truncated = buffer.len() > limit;
    if truncated {
        buffer.truncate(limit);
    }
    let snippet = String::from_utf8(buffer).map_err(|err| ToolRegistryError::ExecutionFailed {
        tool: "web_fetch".to_owned(),
        message: format!("cannot decode response body from `{url}` as UTF-8 text: {err}"),
    })?;
    let suffix = if truncated {
        format!("\n[truncated to {limit} bytes]")
    } else {
        String::new()
    };
    if !status.is_success() {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "web_fetch".to_owned(),
            message: format!(
                "HTTP {} for `{url}` content_type={content_type}\n{snippet}{suffix}",
                status.as_u16()
            ),
        });
    }
    let discovery = arguments
        .iter()
        .find(|argument| argument.name == "domain_plan_ref")
        .and_then(|argument| argument.value.as_str())
        .filter(|domain_plan_ref| !domain_plan_ref.trim().is_empty())
        .map(|domain_plan_ref| {
            web_fetch_search_discovery(
                domain_plan_ref,
                url,
                url,
                snippet.lines().next().unwrap_or(&snippet),
            )
        });
    let discovery = discovery.map(SearchEvidenceDelivery::Discovery);
    Ok(ToolExecutionOutput {
        text: format!(
            "Fetched `{url}` status={} content_type={content_type} bytes={}{}\n{}",
            status.as_u16(),
            snippet.len(),
            if truncated { " truncated=true" } else { "" },
            snippet
        ),
        search_evidence: discovery,
    })
}
fn execute_read_file(arguments: &[ToolArgument]) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let path = required_string(arguments, "read_file", "path")?;
    let root = locked_workspace_root("read_file")?;
    let path = resolve_read_path(&root, path, "read_file", "path")?;
    let metadata = fs::metadata(&path).map_err(|err| ToolRegistryError::ExecutionFailed {
        tool: "read_file".to_owned(),
        message: format!("cannot stat `{}`: {err}", path.display()),
    })?;
    if metadata.is_dir() {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "read_file".to_owned(),
            message: format!(
                "`{}` is a directory, not a file",
                relative_display(&root, &path)
            ),
        });
    }
    let offset = optional_usize(arguments, "read_file", "offset")?.unwrap_or(0);
    let limit = optional_usize(arguments, "read_file", "limit")?.unwrap_or(READ_FILE_DEFAULT_LIMIT);
    if limit == 0 {
        return Err(ToolRegistryError::InvalidArguments {
            tool: "read_file".to_owned(),
            message: "`limit` must be at least 1".to_owned(),
        });
    }
    let text = fs::read_to_string(&path).map_err(|err| ToolRegistryError::ExecutionFailed {
        tool: "read_file".to_owned(),
        message: format!("cannot read `{}` as UTF-8 text: {err}", path.display()),
    })?;
    let lines = text.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return Ok(ToolExecutionOutput {
            text: format!("{}:\n(empty file)", relative_display(&root, &path)),
            search_evidence: None,
        });
    }
    if offset >= lines.len() {
        return Ok(ToolExecutionOutput {
            text: format!(
                "{}:\n(offset {} is past EOF - file has {} lines)",
                relative_display(&root, &path),
                offset,
                lines.len()
            ),
            search_evidence: None,
        });
    }

    let end = offset.saturating_add(limit).min(lines.len());
    let line_width = end.to_string().len().max(1);
    let mut rendered = String::new();
    rendered.push_str(&format!(
        "{} (lines {}-{} of {})\n",
        relative_display(&root, &path),
        offset + 1,
        end,
        lines.len()
    ));
    for (index, line) in lines[offset..end].iter().enumerate() {
        rendered.push_str(&format!(
            "{:>width$}|{}\n",
            offset + index + 1,
            line,
            width = line_width
        ));
    }
    if end < lines.len() {
        rendered.push_str(&format!(
            "\n[more lines below; pass offset={} to continue]",
            end
        ));
    }
    Ok(ToolExecutionOutput {
        text: rendered,
        search_evidence: None,
    })
}

fn execute_write_file(
    arguments: &[ToolArgument],
) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let plan = plan_write_file(arguments)?;
    write_plan(&plan, "write_file")?;
    Ok(ToolExecutionOutput {
        text: plan.success_text,
        search_evidence: None,
    })
}

fn execute_edit_file(arguments: &[ToolArgument]) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let plan = plan_edit_file(arguments)?;
    write_plan(&plan, "edit_file")?;
    Ok(ToolExecutionOutput {
        text: plan.success_text,
        search_evidence: None,
    })
}

fn execute_multi_edit(
    arguments: &[ToolArgument],
) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let plan = plan_multi_edit(arguments)?;
    write_plan(&plan, "multi_edit")?;
    Ok(ToolExecutionOutput {
        text: plan.success_text,
        search_evidence: None,
    })
}

fn plan_write_file(arguments: &[ToolArgument]) -> Result<FileMutationPlan, ToolRegistryError> {
    let path = required_string(arguments, "write_file", "path")?;
    let content = required_present_string(arguments, "write_file", "content")?;
    let root = locked_workspace_root("write_file")?;
    let path = resolve_locked_write_path(&root, path, "write_file", "path")?;
    if path.is_dir() {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "write_file".to_owned(),
            message: format!("`{}` is a directory", relative_display(&root, &path)),
        });
    }
    let before_text = if path.exists() {
        Some(read_text_file(&path, "write_file")?)
    } else {
        None
    };
    let kind = if before_text.is_some() {
        ToolPreviewChangeKind::Modify
    } else {
        ToolPreviewChangeKind::Create
    };
    Ok(FileMutationPlan {
        success_text: format!(
            "{} `{}` ({} bytes)",
            if before_text.is_some() {
                "Overwrote"
            } else {
                "Created"
            },
            relative_display(&root, &path),
            content.len()
        ),
        path: path.clone(),
        preview_change: ToolPreviewFileChange {
            locked_path: path.to_string_lossy().into_owned(),
            kind,
            before_text,
            after_text: Some(content.to_owned()),
        },
    })
}

fn plan_edit_file(arguments: &[ToolArgument]) -> Result<FileMutationPlan, ToolRegistryError> {
    let path = required_string(arguments, "edit_file", "path")?;
    let old_string = required_non_empty_string(arguments, "edit_file", "old_string")?;
    let new_string = required_present_string(arguments, "edit_file", "new_string")?;
    let root = locked_workspace_root("edit_file")?;
    let path = resolve_locked_path(&root, path, "edit_file", "path")?;
    let before_text = read_text_file(&path, "edit_file")?;
    let after_text = replace_exactly_once(&before_text, old_string, new_string, "edit_file")?;
    Ok(FileMutationPlan {
        success_text: format!(
            "Edited `{}` by replacing 1 exact occurrence.",
            relative_display(&root, &path)
        ),
        path: path.clone(),
        preview_change: ToolPreviewFileChange {
            locked_path: path.to_string_lossy().into_owned(),
            kind: ToolPreviewChangeKind::Modify,
            before_text: Some(before_text),
            after_text: Some(after_text),
        },
    })
}

fn plan_multi_edit(arguments: &[ToolArgument]) -> Result<FileMutationPlan, ToolRegistryError> {
    let path = required_string(arguments, "multi_edit", "path")?;
    let steps = parse_multi_edit_steps(arguments)?;
    let root = locked_workspace_root("multi_edit")?;
    let path = resolve_locked_path(&root, path, "multi_edit", "path")?;
    let before_text = read_text_file(&path, "multi_edit")?;
    let mut after_text = before_text.clone();
    let mut total_replacements = 0usize;
    for (index, step) in steps.iter().enumerate() {
        let replacements = if step.replace_all {
            replace_all_occurrences(
                &mut after_text,
                &step.old_string,
                &step.new_string,
                "multi_edit",
                index,
            )?
        } else {
            after_text = replace_exactly_once(
                &after_text,
                &step.old_string,
                &step.new_string,
                "multi_edit",
            )?;
            1
        };
        total_replacements += replacements;
    }
    Ok(FileMutationPlan {
        success_text: format!(
            "Edited `{}` with {} edit(s) and {} replacement(s).",
            relative_display(&root, &path),
            steps.len(),
            total_replacements
        ),
        path: path.clone(),
        preview_change: ToolPreviewFileChange {
            locked_path: path.to_string_lossy().into_owned(),
            kind: ToolPreviewChangeKind::Modify,
            before_text: Some(before_text),
            after_text: Some(after_text),
        },
    })
}

fn parse_multi_edit_steps(
    arguments: &[ToolArgument],
) -> Result<Vec<MultiEditStep>, ToolRegistryError> {
    let edits = argument_value(arguments, "edits")
        .and_then(Value::as_array)
        .ok_or_else(|| ToolRegistryError::InvalidArguments {
            tool: "multi_edit".to_owned(),
            message: "`edits` array is required".to_owned(),
        })?;
    if edits.is_empty() {
        return Err(ToolRegistryError::InvalidArguments {
            tool: "multi_edit".to_owned(),
            message: "`edits` must contain at least one item".to_owned(),
        });
    }

    edits
        .iter()
        .enumerate()
        .map(|(index, edit)| {
            let object = edit.as_object().ok_or_else(|| {
                invalid_tool_argument("multi_edit", index, "edit must be an object")
            })?;
            let old_string = object
                .get("old_string")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if old_string.is_empty() {
                return Err(invalid_tool_argument(
                    "multi_edit",
                    index,
                    "`old_string` is required",
                ));
            }
            let new_string = object
                .get("new_string")
                .and_then(Value::as_str)
                .ok_or_else(|| {
                    invalid_tool_argument("multi_edit", index, "`new_string` is required")
                })?;
            let replace_all = object
                .get("replace_all")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            Ok(MultiEditStep {
                old_string: old_string.to_owned(),
                new_string: new_string.to_owned(),
                replace_all,
            })
        })
        .collect()
}

fn plan_delete_range(arguments: &[ToolArgument]) -> Result<FileMutationPlan, ToolRegistryError> {
    let path = required_string(arguments, "delete_range", "path")?;
    let start_anchor = required_string(arguments, "delete_range", "start_anchor")?;
    let end_anchor = required_string(arguments, "delete_range", "end_anchor")?;
    let inclusive = argument_value(arguments, "inclusive")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let root = locked_workspace_root("delete_range")?;
    let path = resolve_locked_path(&root, path, "delete_range", "path")?;
    let before_text = read_text_file(&path, "delete_range")?;
    let start_pos =
        before_text
            .find(start_anchor)
            .ok_or_else(|| ToolRegistryError::ExecutionFailed {
                tool: "delete_range".to_owned(),
                message: format!("start anchor not found: `{}`", start_anchor),
            })?;
    let end_pos =
        before_text
            .find(end_anchor)
            .ok_or_else(|| ToolRegistryError::ExecutionFailed {
                tool: "delete_range".to_owned(),
                message: format!("end anchor not found: `{}`", end_anchor),
            })?;
    if end_pos < start_pos {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "delete_range".to_owned(),
            message: "end anchor must not appear before start anchor".to_owned(),
        });
    }
    let after_text = if inclusive {
        format!(
            "{}{}",
            &before_text[..start_pos],
            &before_text[end_pos + end_anchor.len()..]
        )
    } else {
        // Exclusive mode: keep text before start_anchor and from end_anchor onward.
        // This removes the range between the two anchors while preserving both anchors.
        format!(
            "{}{}",
            &before_text[..start_pos + start_anchor.len()],
            &before_text[end_pos..]
        )
    };
    if after_text == before_text {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "delete_range".to_owned(),
            message: "delete range produced no change".to_owned(),
        });
    }
    let changed_lines = before_text.lines().count() - after_text.lines().count();
    Ok(FileMutationPlan {
        success_text: format!(
            "Deleted range in `{}` ({} line(s) removed).",
            relative_display(&root, &path),
            changed_lines.max(1)
        ),
        path: path.clone(),
        preview_change: ToolPreviewFileChange {
            locked_path: path.to_string_lossy().into_owned(),
            kind: ToolPreviewChangeKind::Delete,
            before_text: Some(before_text),
            after_text: Some(after_text),
        },
    })
}

fn execute_delete_range(
    arguments: &[ToolArgument],
) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let plan = plan_delete_range(arguments)?;
    write_plan(&plan, "delete_range")?;
    Ok(ToolExecutionOutput {
        text: plan.success_text,
        search_evidence: None,
    })
}

fn execute_glob(arguments: &[ToolArgument]) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let pattern = required_string(arguments, "glob", "pattern")?;
    let root = locked_workspace_root("glob")?;
    let joined_pattern = resolve_glob_pattern(&root, pattern)?;
    let joined_pattern = joined_pattern.to_string_lossy().to_string();
    let mut matches = glob::glob(&joined_pattern)
        .map_err(|err| ToolRegistryError::InvalidArguments {
            tool: "glob".to_owned(),
            message: format!("invalid pattern `{pattern}`: {err}"),
        })?
        .filter_map(Result::ok)
        .filter(|path| path.starts_with(&root))
        .collect::<Vec<_>>();

    let mut truncated = false;
    if matches.is_empty() && !contains_path_separator(pattern) {
        let basename_pattern =
            Pattern::new(pattern).map_err(|err| ToolRegistryError::InvalidArguments {
                tool: "glob".to_owned(),
                message: format!("invalid pattern `{pattern}`: {err}"),
            })?;
        for entry in WalkDir::new(&root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| !should_skip_walk_entry(entry))
        {
            let entry = entry.map_err(|err| ToolRegistryError::ExecutionFailed {
                tool: "glob".to_owned(),
                message: err.to_string(),
            })?;
            if entry.file_type().is_file()
                && basename_pattern.matches(entry.file_name().to_string_lossy().as_ref())
            {
                matches.push(entry.path().to_path_buf());
                if matches.len() >= GLOB_MAX_RESULTS {
                    truncated = true;
                    break;
                }
            }
        }
    }

    matches.sort();
    matches.dedup();
    if matches.is_empty() {
        return Ok(ToolExecutionOutput {
            text: "(no matches)".to_owned(),
            search_evidence: None,
        });
    }

    truncated |= matches.len() > GLOB_MAX_RESULTS;
    let shown = if matches.len() > GLOB_MAX_RESULTS {
        &matches[..GLOB_MAX_RESULTS]
    } else {
        matches.as_slice()
    };
    let mut text = shown
        .iter()
        .map(|path| display_glob_path(&root, path))
        .collect::<Vec<_>>()
        .join("\n");
    if truncated {
        text.push_str(&format!(
            "\n... (truncated at {} results)",
            GLOB_MAX_RESULTS
        ));
    }
    Ok(ToolExecutionOutput {
        text,
        search_evidence: None,
    })
}

fn execute_grep(arguments: &[ToolArgument]) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let pattern = required_string(arguments, "grep", "pattern")?;
    let regex = Regex::new(pattern).map_err(|err| ToolRegistryError::InvalidArguments {
        tool: "grep".to_owned(),
        message: format!("invalid pattern `{pattern}`: {err}"),
    })?;
    let target = argument_value(arguments, "path")
        .and_then(Value::as_str)
        .unwrap_or(".")
        .trim();
    let root = locked_workspace_root("grep")?;
    let target = resolve_read_path(&root, target, "grep", "path")?;
    let metadata = fs::metadata(&target).map_err(|err| ToolRegistryError::ExecutionFailed {
        tool: "grep".to_owned(),
        message: format!("cannot stat `{}`: {err}", target.display()),
    })?;

    let mut matches = Vec::new();
    let mut truncated = false;
    if metadata.is_dir() {
        for entry in WalkDir::new(&target)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| !should_skip_walk_entry(entry))
        {
            let entry = entry.map_err(|err| ToolRegistryError::ExecutionFailed {
                tool: "grep".to_owned(),
                message: err.to_string(),
            })?;
            if !entry.file_type().is_file() {
                continue;
            }
            if let Some(file_matches) = grep_file(&root, entry.path(), &regex, false)? {
                matches.extend(file_matches);
            }
            if matches.len() >= GREP_MAX_MATCHES {
                truncated = true;
                break;
            }
        }
    } else if let Some(file_matches) = grep_file(&root, &target, &regex, true)? {
        if file_matches.len() >= GREP_MAX_MATCHES {
            truncated = true;
        }
        matches.extend(file_matches);
    }

    if matches.is_empty() {
        return Ok(ToolExecutionOutput {
            text: "(no matches)".to_owned(),
            search_evidence: None,
        });
    }
    truncated |= matches.len() > GREP_MAX_MATCHES;
    let shown = if matches.len() > GREP_MAX_MATCHES {
        &matches[..GREP_MAX_MATCHES]
    } else {
        matches.as_slice()
    };
    let mut text = shown.join("\n");
    if truncated {
        text.push_str(&format!(
            "\n... (truncated at {} matches)",
            GREP_MAX_MATCHES
        ));
    }
    Ok(ToolExecutionOutput {
        text,
        search_evidence: None,
    })
}

fn execute_ls(arguments: &[ToolArgument]) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let raw_path = argument_value(arguments, "path")
        .and_then(Value::as_str)
        .unwrap_or(".")
        .trim();
    let recursive = optional_bool(arguments, "ls", "recursive")?.unwrap_or(false);
    let root = locked_workspace_root("ls")?;
    let path = resolve_read_path(&root, raw_path, "ls", "path")?;
    let metadata = fs::metadata(&path).map_err(|err| ToolRegistryError::ExecutionFailed {
        tool: "ls".to_owned(),
        message: format!("cannot stat `{}`: {err}", path.display()),
    })?;
    if !metadata.is_dir() {
        let size = metadata.len();
        return Ok(ToolExecutionOutput {
            text: format!("{}\t{size}", relative_display(&root, &path)),
            search_evidence: None,
        });
    }

    if recursive {
        let mut rows = Vec::new();
        for entry in WalkDir::new(&path)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| !should_skip_walk_entry(entry))
        {
            let entry = entry.map_err(|err| ToolRegistryError::ExecutionFailed {
                tool: "ls".to_owned(),
                message: err.to_string(),
            })?;
            if entry.depth() == 0 {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&path)
                .unwrap_or(entry.path())
                .to_string_lossy()
                .replace('\\', "/");
            if entry.file_type().is_dir() {
                rows.push(format!("{rel}/"));
            } else {
                let size = entry.metadata().map(|meta| meta.len()).unwrap_or(0);
                rows.push(format!("{rel}\t{size}"));
            }
        }
        if rows.is_empty() {
            return Ok(ToolExecutionOutput {
                text: "(empty directory tree)".to_owned(),
                search_evidence: None,
            });
        }
        return Ok(ToolExecutionOutput {
            text: rows.join("\n"),
            search_evidence: None,
        });
    }

    let mut rows = fs::read_dir(&path)
        .map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: "ls".to_owned(),
            message: format!("cannot list `{}`: {err}", path.display()),
        })?
        .map(|entry| {
            let entry = entry.map_err(|err| ToolRegistryError::ExecutionFailed {
                tool: "ls".to_owned(),
                message: err.to_string(),
            })?;
            let file_name = entry.file_name().to_string_lossy().into_owned();
            let metadata = entry
                .metadata()
                .map_err(|err| ToolRegistryError::ExecutionFailed {
                    tool: "ls".to_owned(),
                    message: err.to_string(),
                })?;
            if metadata.is_dir() {
                Ok(format!("{file_name}/"))
            } else {
                Ok(format!("{file_name}\t{}", metadata.len()))
            }
        })
        .collect::<Result<Vec<_>, ToolRegistryError>>()?;
    rows.sort();
    if rows.is_empty() {
        return Ok(ToolExecutionOutput {
            text: "(empty directory)".to_owned(),
            search_evidence: None,
        });
    }
    Ok(ToolExecutionOutput {
        text: rows.join("\n"),
        search_evidence: None,
    })
}

fn required_string<'a>(
    arguments: &'a [ToolArgument],
    tool: &str,
    field: &str,
) -> Result<&'a str, ToolRegistryError> {
    let value = argument_value(arguments, field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if value.is_empty() {
        return Err(ToolRegistryError::InvalidArguments {
            tool: tool.to_owned(),
            message: format!("`{field}` is required"),
        });
    }
    Ok(value)
}

fn required_non_empty_string<'a>(
    arguments: &'a [ToolArgument],
    tool: &str,
    field: &str,
) -> Result<&'a str, ToolRegistryError> {
    let value = required_string(arguments, tool, field)?;
    if value.is_empty() {
        return Err(ToolRegistryError::InvalidArguments {
            tool: tool.to_owned(),
            message: format!("`{field}` may not be empty"),
        });
    }
    Ok(value)
}

fn required_present_string<'a>(
    arguments: &'a [ToolArgument],
    tool: &str,
    field: &str,
) -> Result<&'a str, ToolRegistryError> {
    argument_value(arguments, field)
        .and_then(Value::as_str)
        .ok_or_else(|| ToolRegistryError::InvalidArguments {
            tool: tool.to_owned(),
            message: format!("`{field}` is required"),
        })
}

fn argument_value<'a>(arguments: &'a [ToolArgument], name: &str) -> Option<&'a Value> {
    arguments
        .iter()
        .find(|argument| argument.name == name)
        .map(|argument| &argument.value)
}

fn optional_usize(
    arguments: &[ToolArgument],
    tool: &str,
    field: &str,
) -> Result<Option<usize>, ToolRegistryError> {
    let Some(value) = argument_value(arguments, field) else {
        return Ok(None);
    };
    let number = value
        .as_u64()
        .ok_or_else(|| ToolRegistryError::InvalidArguments {
            tool: tool.to_owned(),
            message: format!("`{field}` must be a non-negative integer"),
        })?;
    usize::try_from(number)
        .map(Some)
        .map_err(|_| ToolRegistryError::InvalidArguments {
            tool: tool.to_owned(),
            message: format!("`{field}` is too large"),
        })
}

fn optional_bool(
    arguments: &[ToolArgument],
    tool: &str,
    field: &str,
) -> Result<Option<bool>, ToolRegistryError> {
    let Some(value) = argument_value(arguments, field) else {
        return Ok(None);
    };
    value
        .as_bool()
        .map(Some)
        .ok_or_else(|| ToolRegistryError::InvalidArguments {
            tool: tool.to_owned(),
            message: format!("`{field}` must be a boolean"),
        })
}

fn invalid_tool_argument(tool: &str, index: usize, message: &str) -> ToolRegistryError {
    ToolRegistryError::InvalidArguments {
        tool: tool.to_owned(),
        message: format!("item {}: {message}", index + 1),
    }
}

fn locked_workspace_root(tool: &str) -> Result<PathBuf, ToolRegistryError> {
    if let Some(root) = TOOL_WORKSPACE_ROOT.with(|slot| slot.borrow().clone()) {
        return Ok(root);
    }
    locked_workspace_root_from_env(
        tool,
        env::var_os("FREEHAND_WORKSPACE_ROOT").or_else(|| env::var_os("FREEHAND_DAEMON_WORKDIR")),
    )
}

fn locked_workspace_root_from_env(
    tool: &str,
    configured_root: Option<OsString>,
) -> Result<PathBuf, ToolRegistryError> {
    let root = if let Some(path) = configured_root {
        PathBuf::from(path)
    } else {
        env::current_dir().map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: tool.to_owned(),
            message: format!("cannot read current working directory: {err}"),
        })?
    };
    fs::canonicalize(root).map_err(|err| ToolRegistryError::ExecutionFailed {
        tool: tool.to_owned(),
        message: format!("cannot canonicalize current working directory: {err}"),
    })
}

fn resolve_locked_path(
    root: &Path,
    raw: &str,
    tool: &str,
    field: &str,
) -> Result<PathBuf, ToolRegistryError> {
    let candidate = absolutize_tool_path(root, raw);
    let canonical =
        fs::canonicalize(&candidate).map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: tool.to_owned(),
            message: format!(
                "cannot resolve `{field}` `{raw}`: {err}\n{}",
                path_resolution_diagnostic(root, field, raw)
            ),
        })?;
    if !canonical.starts_with(root) {
        return Err(ToolRegistryError::WorkspaceBoundaryViolation {
            tool: tool.to_owned(),
            field: field.to_owned(),
            root: root.to_string_lossy().into_owned(),
            target: canonical.to_string_lossy().into_owned(),
        });
    }
    Ok(canonical)
}

fn resolve_read_path(
    root: &Path,
    raw: &str,
    tool: &str,
    field: &str,
) -> Result<PathBuf, ToolRegistryError> {
    let candidate = absolutize_tool_path(root, raw);
    let canonical =
        fs::canonicalize(&candidate).map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: tool.to_owned(),
            message: format!(
                "cannot resolve `{field}` `{raw}`: {err}\n{}",
                path_resolution_diagnostic(root, field, raw)
            ),
        })?;
    if !canonical.starts_with(root) {
        return Err(ToolRegistryError::WorkspaceBoundaryViolation {
            tool: tool.to_owned(),
            field: field.to_owned(),
            root: root.to_string_lossy().into_owned(),
            target: canonical.to_string_lossy().into_owned(),
        });
    }
    Ok(canonical)
}

fn resolve_locked_write_path(
    root: &Path,
    raw: &str,
    tool: &str,
    field: &str,
) -> Result<PathBuf, ToolRegistryError> {
    let candidate = absolutize_tool_path(root, raw);
    let file_name = candidate
        .file_name()
        .ok_or_else(|| ToolRegistryError::InvalidArguments {
            tool: tool.to_owned(),
            message: format!("`{field}` must point to a file path"),
        })?;
    let parent = candidate.parent().unwrap_or(root);
    let canonical_parent =
        fs::canonicalize(parent).map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: tool.to_owned(),
            message: format!(
                "cannot resolve parent directory for `{field}` `{raw}`: {err}\n{}",
                path_resolution_diagnostic(root, field, raw)
            ),
        })?;
    if !canonical_parent.starts_with(root) {
        return Err(ToolRegistryError::WorkspaceBoundaryViolation {
            tool: tool.to_owned(),
            field: field.to_owned(),
            root: root.to_string_lossy().into_owned(),
            target: candidate.to_string_lossy().into_owned(),
        });
    }
    Ok(canonical_parent.join(file_name))
}

fn resolve_glob_pattern(root: &Path, pattern: &str) -> Result<PathBuf, ToolRegistryError> {
    let path = expand_leading_tilde_path(pattern);
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ToolRegistryError::InvalidArguments {
            tool: "glob".to_owned(),
            message: "pattern may not contain `..`".to_owned(),
        });
    }
    if path.is_absolute() {
        let resolved =
            canonicalize_absolute_glob_pattern(&path).unwrap_or_else(|| path.to_path_buf());
        if !resolved.starts_with(root) {
            return Err(ToolRegistryError::WorkspaceBoundaryViolation {
                tool: "glob".to_owned(),
                field: "pattern".to_owned(),
                root: root.to_string_lossy().into_owned(),
                target: pattern.to_owned(),
            });
        }
        return Ok(resolved);
    }
    Ok(root.join(path))
}

fn canonicalize_absolute_glob_pattern(pattern: &Path) -> Option<PathBuf> {
    let pattern_text = pattern.to_string_lossy();
    let first_meta = pattern_text
        .find(['*', '?', '[', '{'])
        .unwrap_or(pattern_text.len());
    let prefix_end = pattern_text[..first_meta]
        .rfind(['/', '\\'])
        .map(|index| index + 1)
        .unwrap_or(first_meta);
    let prefix = &pattern_text[..prefix_end];
    if prefix.is_empty() {
        return None;
    }
    let prefix_path = PathBuf::from(prefix);
    let canonical_prefix = fs::canonicalize(&prefix_path).ok().or_else(|| {
        let nearest_existing = nearest_existing_path(&prefix_path)?;
        let nearest_existing_canonical = fs::canonicalize(&nearest_existing).ok()?;
        let missing_suffix = prefix_path.strip_prefix(&nearest_existing).ok()?;
        Some(nearest_existing_canonical.join(missing_suffix))
    })?;
    let suffix = &pattern_text[prefix_end..];
    if suffix.is_empty() {
        Some(canonical_prefix)
    } else {
        Some(canonical_prefix.join(suffix))
    }
}

fn absolutize_tool_path(root: &Path, raw: &str) -> PathBuf {
    let expanded = expand_leading_tilde_path(raw);
    if expanded.is_absolute() {
        expanded
    } else {
        root.join(expanded)
    }
}

fn expand_leading_tilde_path(path: &str) -> PathBuf {
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
    env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(PathBuf::from)
}

fn nearest_existing_path(path: &Path) -> Option<PathBuf> {
    let mut candidate = path.to_path_buf();
    loop {
        if candidate.exists() {
            return Some(candidate);
        }
        if !candidate.pop() {
            return None;
        }
    }
}

fn symlink_ancestors(path: &Path) -> Vec<PathSymlinkDiagnostic> {
    let mut current = PathBuf::new();
    let mut symlinks = Vec::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => current.push(prefix.as_os_str()),
            Component::RootDir => current.push(component.as_os_str()),
            Component::CurDir => current.push(component.as_os_str()),
            Component::ParentDir | Component::Normal(_) => current.push(component.as_os_str()),
        }
        if current.as_os_str().is_empty() {
            continue;
        }
        let Ok(metadata) = fs::symlink_metadata(&current) else {
            continue;
        };
        if metadata.file_type().is_symlink()
            && let Ok(target) = fs::read_link(&current)
        {
            symlinks.push(PathSymlinkDiagnostic {
                path: current.clone(),
                target,
            });
        }
    }
    symlinks
}

fn path_resolution_diagnostic(root: &Path, field: &str, raw: &str) -> String {
    PathResolutionDiagnostic::inspect(root, raw).render(field)
}

fn contains_path_separator(pattern: &str) -> bool {
    pattern.contains('/') || pattern.contains('\\')
}

fn should_skip_walk_entry(entry: &DirEntry) -> bool {
    entry.file_type().is_dir()
        && matches!(
            entry.file_name().to_string_lossy().as_ref(),
            ".git" | "node_modules" | "target" | "__pycache__" | ".idea" | ".vscode"
        )
}

fn display_glob_path(root: &Path, path: &Path) -> String {
    let display = relative_display(root, path);
    if path.is_dir() {
        format!("{display}/")
    } else {
        display
    }
}

fn relative_display(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn read_text_file(path: &Path, tool: &str) -> Result<String, ToolRegistryError> {
    fs::read_to_string(path).map_err(|err| ToolRegistryError::ExecutionFailed {
        tool: tool.to_owned(),
        message: format!("cannot read `{}` as UTF-8 text: {err}", path.display()),
    })
}

fn temp_tool_output_path(tool: &str) -> Result<PathBuf, ToolRegistryError> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: tool.to_owned(),
            message: format!("cannot derive temp file timestamp: {err}"),
        })?
        .as_nanos();
    Ok(env::temp_dir().join(format!(
        "freehand-{tool}-{}-{unique}.log",
        std::process::id()
    )))
}

fn read_command_output(path: &Path, tool: &str) -> Result<String, ToolRegistryError> {
    let output = fs::read_to_string(path).map_err(|err| ToolRegistryError::ExecutionFailed {
        tool: tool.to_owned(),
        message: format!(
            "cannot read command output `{}` as UTF-8 text: {err}",
            path.display()
        ),
    })?;
    let _ = fs::remove_file(path);
    Ok(output)
}

fn render_shell_output(output: String) -> String {
    if output.is_empty() {
        "(no output)".to_owned()
    } else {
        output
    }
}

fn render_shell_output_suffix(output: &str) -> String {
    if output.is_empty() {
        String::new()
    } else {
        format!("\n\n{output}")
    }
}

fn write_plan(plan: &FileMutationPlan, tool: &str) -> Result<(), ToolRegistryError> {
    let content = plan.preview_change.after_text.as_deref().ok_or_else(|| {
        ToolRegistryError::ExecutionFailed {
            tool: tool.to_owned(),
            message: "preview plan is missing post-image text".to_owned(),
        }
    })?;
    write_text_atomic(&plan.path, content, tool)
}

fn write_text_atomic(path: &Path, content: &str, tool: &str) -> Result<(), ToolRegistryError> {
    let parent = path
        .parent()
        .ok_or_else(|| ToolRegistryError::ExecutionFailed {
            tool: tool.to_owned(),
            message: format!("cannot determine parent directory for `{}`", path.display()),
        })?;
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: tool.to_owned(),
            message: format!("cannot derive temp file timestamp: {err}"),
        })?
        .as_nanos();
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("freehand-tool-target");
    let temp_path = parent.join(format!(".{file_name}.freehand-tmp-{unique}"));
    fs::write(&temp_path, content).map_err(|err| ToolRegistryError::ExecutionFailed {
        tool: tool.to_owned(),
        message: format!("cannot write temp file `{}`: {err}", temp_path.display()),
    })?;
    if let Err(err) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(ToolRegistryError::ExecutionFailed {
            tool: tool.to_owned(),
            message: format!(
                "cannot replace `{}` with temp file `{}`: {err}",
                path.display(),
                temp_path.display()
            ),
        });
    }
    Ok(())
}

fn replace_exactly_once(
    haystack: &str,
    old_string: &str,
    new_string: &str,
    tool: &str,
) -> Result<String, ToolRegistryError> {
    let matches = haystack.match_indices(old_string).collect::<Vec<_>>();
    match matches.len() {
        0 => Err(ToolRegistryError::ExecutionFailed {
            tool: tool.to_owned(),
            message: "target text not found exactly once".to_owned(),
        }),
        1 => Ok(haystack.replacen(old_string, new_string, 1)),
        count => Err(ToolRegistryError::ExecutionFailed {
            tool: tool.to_owned(),
            message: format!(
                "target text matched {count} times; use `multi_edit` with `replace_all=true` or choose a more specific string"
            ),
        }),
    }
}

fn replace_all_occurrences(
    content: &mut String,
    old_string: &str,
    new_string: &str,
    tool: &str,
    index: usize,
) -> Result<usize, ToolRegistryError> {
    let matches = content.match_indices(old_string).count();
    if matches == 0 {
        return Err(invalid_tool_argument(
            tool,
            index,
            "target text not found for replace_all edit",
        ));
    }
    *content = content.replace(old_string, new_string);
    Ok(matches)
}

fn grep_file(
    root: &Path,
    path: &Path,
    regex: &Regex,
    strict: bool,
) -> Result<Option<Vec<String>>, ToolRegistryError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if strict => {
            return Err(ToolRegistryError::ExecutionFailed {
                tool: "grep".to_owned(),
                message: format!("cannot read `{}` as UTF-8 text: {err}", path.display()),
            });
        }
        Err(_) => return Ok(None),
    };
    let mut matches = Vec::new();
    let display = relative_display(root, path);
    for (index, line) in text.lines().enumerate() {
        if regex.is_match(line) {
            matches.push(format!("{display}:{}:{line}", index + 1));
            if matches.len() >= GREP_MAX_MATCHES {
                break;
            }
        }
    }
    if matches.is_empty() {
        Ok(None)
    } else {
        Ok(Some(matches))
    }
}

pub fn rendered_tool_arguments(arguments: &[ToolArgument]) -> Result<String, ToolRegistryError> {
    render_tool_arguments_json(arguments).map_err(|err| ToolRegistryError::InvalidArguments {
        tool: "tool_arguments".to_owned(),
        message: err.to_string(),
    })
}

fn canonicalize_tool_definition(definition: &ProviderToolDefinition) -> String {
    format!(
        "{}|{}|{}",
        definition.name,
        definition.description,
        canonicalize_json(&definition.input_schema)
    )
}

fn canonicalize_json(value: &Value) -> String {
    match value {
        Value::Null => "null".to_owned(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => serde_json::to_string(value).expect("json string canonicalization"),
        Value::Array(values) => format!(
            "[{}]",
            values
                .iter()
                .map(canonicalize_json)
                .collect::<Vec<_>>()
                .join(",")
        ),
        Value::Object(map) => {
            let mut keys = map.keys().cloned().collect::<Vec<_>>();
            keys.sort();
            let pairs = keys
                .into_iter()
                .map(|key| {
                    let value = map
                        .get(&key)
                        .expect("canonicalize object value for existing key");
                    format!(
                        "{}:{}",
                        serde_json::to_string(&key).expect("json object key canonicalization"),
                        canonicalize_json(value)
                    )
                })
                .collect::<Vec<_>>();
            format!("{{{}}}", pairs.join(","))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::{
        AgentId, FeatureId, SessionId, ToolCallContract, ToolCallId, TraceId, TurnId,
    };
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::{Mutex, OnceLock};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reasonix_aligned_registry_exports_core_tool_names() {
        let registry = BuiltinToolRegistry::reasonix_aligned();
        let names = registry
            .definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();
        assert!(names.contains(&"bash".to_owned()));
        assert!(names.contains(&"read_file".to_owned()));
        assert!(names.contains(&"write_file".to_owned()));
        assert!(names.contains(&"edit_file".to_owned()));
        assert!(names.contains(&"multi_edit".to_owned()));
        assert!(names.contains(&"grep".to_owned()));
        assert!(names.contains(&"glob".to_owned()));
        assert!(names.contains(&"ls".to_owned()));
        assert!(names.contains(&"todo_write".to_owned()));
        assert!(names.contains(&"complete_step".to_owned()));
        assert!(names.contains(&"timer".to_owned()));
        assert!(names.contains(&"web_fetch".to_owned()));
        assert_eq!(registry.read_only("read_file"), Some(true));
        assert_eq!(registry.read_only("glob"), Some(true));
        assert_eq!(registry.read_only("grep"), Some(true));
        assert_eq!(registry.read_only("ls"), Some(true));
        assert_eq!(registry.read_only("todo_write"), Some(true));
        assert_eq!(registry.read_only("web_fetch"), Some(true));
    }

    #[test]
    fn master_tool_surface_excludes_unsandboxed_shell() {
        let registry = BuiltinToolRegistry::reasonix_aligned();
        let names = registry
            .master_implemented_definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();

        assert!(!names.contains(&"bash".to_owned()));
        assert!(names.contains(&"task".to_owned()));
        assert!(names.contains(&"timer".to_owned()));
        for local_tool in [
            "read_file",
            "write_file",
            "edit_file",
            "multi_edit",
            "delete_range",
            "grep",
            "glob",
            "ls",
        ] {
            assert!(
                names.contains(&local_tool.to_owned()),
                "master surface must expose local workspace tool {local_tool}: {names:?}"
            );
        }
        assert!(names.contains(&"web_fetch".to_owned()));
        assert_eq!(
            names.len(),
            12,
            "master surface must contain local workspace tools plus task/timer/web_fetch/camo"
        );
        for forbidden in ["todo_write", "complete_step", "bash"] {
            assert!(
                !names.contains(&forbidden.to_owned()),
                "master surface must not expose {forbidden}: {names:?}"
            );
        }
        assert_eq!(
            registry.execution_scope("bash"),
            Some(BuiltinToolExecutionScope::Shell)
        );
        assert_eq!(
            registry.execution_scope("read_file"),
            Some(BuiltinToolExecutionScope::Workspace)
        );
        assert_eq!(
            registry.execution_scope("task"),
            Some(BuiltinToolExecutionScope::Framework)
        );
        assert_eq!(
            registry.execution_scope("timer"),
            Some(BuiltinToolExecutionScope::Framework)
        );
        assert_eq!(
            registry.execution_scope("web_fetch"),
            Some(BuiltinToolExecutionScope::Network)
        );
    }

    #[test]
    fn registry_projection_exposes_safe_tool_guidance_without_local_web_search() {
        let registry = BuiltinToolRegistry::reasonix_aligned();
        let projection = registry.registry_projection();
        let names = projection
            .tools
            .iter()
            .map(|tool| tool.name.as_str())
            .collect::<Vec<_>>();
        let find = |name: &str| {
            projection
                .tools
                .iter()
                .find(|tool| tool.name == name)
                .unwrap_or_else(|| panic!("missing projected tool {name}: {names:?}"))
        };

        assert_eq!(projection.registry_version, "reasonix-aligned-v1");
        assert!(
            projection
                .guidance
                .iter()
                .any(|line| line.contains("exact JSON schema"))
        );
        assert!(
            projection
                .guidance
                .iter()
                .any(|line| line.contains("absolute or symlink paths"))
        );
        assert!(
            projection
                .guidance
                .iter()
                .any(|line| line.contains("not a Freehand local function tool named web_search"))
        );
        assert!(!names.contains(&"web_search"));

        let task = find("task");
        assert!(task.implemented);
        assert!(task.exposed_to_master);
        assert!(!task.exposed_to_worker);
        assert_eq!(task.execution_scope, "framework");
        assert!(task.input_schema.to_string().contains("\"op\""));

        let timer = find("timer");
        assert!(timer.implemented);
        assert!(timer.exposed_to_master);
        assert!(!timer.exposed_to_worker);
        assert_eq!(timer.execution_scope, "framework");
        assert!(
            timer
                .guidance
                .iter()
                .any(|line| line.contains("dead-waiting"))
        );

        let web_fetch = find("web_fetch");
        assert!(web_fetch.read_only);
        assert!(web_fetch.exposed_to_master);
        assert!(web_fetch.exposed_to_worker);
        assert_eq!(web_fetch.execution_scope, "network");
        assert!(
            web_fetch
                .guidance
                .iter()
                .any(|line| line.contains("not broad search"))
        );

        let bash = find("bash");
        assert!(bash.implemented);
        assert!(!bash.exposed_to_master);
        assert!(!bash.exposed_to_worker);
        assert_eq!(bash.execution_scope, "shell");

        for worker_only in ["todo_write", "complete_step"] {
            let tool = find(worker_only);
            assert!(tool.exposed_to_worker);
            assert!(!tool.exposed_to_master);
            assert_eq!(tool.execution_scope, "framework");
        }

        for path_tool in ["glob", "read_file", "ls"] {
            let tool = find(path_tool);
            let text = format!(
                "{}\n{}\n{}",
                tool.description,
                tool.examples.join("\n"),
                tool.guidance.join("\n")
            );
            assert!(text.contains("locked workspace"), "{path_tool}: {text}");
            assert!(text.contains("absolute"), "{path_tool}: {text}");
            assert!(text.contains("symlink"), "{path_tool}: {text}");
            assert!(
                text.contains("Leading-~") || text.contains("leading `~`"),
                "{path_tool}: {text}"
            );
        }
    }

    #[test]
    fn timer_tool_schema_exposes_internal_schedule_contract() {
        let registry = BuiltinToolRegistry::reasonix_aligned();
        let timer = registry
            .definitions()
            .into_iter()
            .find(|definition| definition.name == "timer")
            .expect("timer definition");
        let properties = timer
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .expect("timer properties");
        assert_eq!(timer.input_schema.get("required"), Some(&json!(["op"])));
        assert_eq!(
            properties.get("op").and_then(|schema| schema.get("enum")),
            Some(&json!(["schedule", "cancel", "list"]))
        );
        assert_eq!(
            properties.get("mode").and_then(|schema| schema.get("enum")),
            Some(&json!(["relative", "absolute", "recurring"]))
        );
        assert!(properties.contains_key("prompt"));
        assert!(properties.contains_key("delay_seconds"));
        assert!(properties.contains_key("run_at_unix_seconds"));
        let repeat_properties = properties
            .get("repeat")
            .and_then(|schema| schema.get("properties"))
            .and_then(Value::as_object)
            .expect("repeat properties");
        assert_eq!(
            repeat_properties
                .get("kind")
                .and_then(|schema| schema.get("enum")),
            Some(&json!(["interval", "daily", "weekly", "cron"]))
        );
        assert!(timer.input_schema.get("examples").is_some());
        assert!(repeat_properties.contains_key("time_of_day_seconds_local"));
        assert!(repeat_properties.contains_key("expression"));
        assert!(repeat_properties.contains_key("weekdays"));
        assert!(repeat_properties.contains_key("skip_weekends"));
        assert!(repeat_properties.contains_key("max_runs"));
        let schema_text = timer.input_schema.to_string();
        assert!(timer.description.contains("exceeds 3 minutes"));
        assert!(timer.description.contains("dead-waiting"));
        assert!(
            timer
                .description
                .contains("continue any other ready Master-side work")
        );
        assert!(
            timer
                .description
                .contains("Do not claim a timer was scheduled")
        );
        assert!(timer.description.contains("Timer scheduled"));
        assert!(schema_text.contains("what waited condition to revisit"));
        assert!(schema_text.contains("waiting more than 3 minutes"));
    }

    #[test]
    fn glob_tool_schema_guides_workspace_scoped_paths_without_trial_calls() {
        let registry = BuiltinToolRegistry::reasonix_aligned();
        let glob = registry
            .definitions()
            .into_iter()
            .find(|definition| definition.name == "glob")
            .expect("glob definition");
        let pattern_description = glob
            .input_schema
            .get("properties")
            .and_then(Value::as_object)
            .and_then(|properties| properties.get("pattern"))
            .and_then(|schema| schema.get("description"))
            .and_then(Value::as_str)
            .expect("glob pattern description");

        assert!(glob.description.contains("locked workspace"));
        assert!(glob.description.contains("leading `~` is expanded"));
        assert!(
            glob.description
                .contains("resolves inside the locked workspace")
        );
        assert!(glob.description.contains("use `ls`"));
        assert!(glob.description.contains("read_file"));
        assert!(pattern_description.contains("Workspace-scoped glob"));
        assert!(pattern_description.contains("leading-~ patterns"));
        assert!(pattern_description.contains("\"../**\""));
        assert!(pattern_description.contains("outside the locked workspace"));
    }

    #[test]
    fn file_tool_schemas_guide_worker_away_from_observed_bad_calls() {
        let registry = BuiltinToolRegistry::reasonix_aligned();
        let definitions = registry.definitions();
        let read_file = definitions
            .iter()
            .find(|definition| definition.name == "read_file")
            .expect("read_file definition");
        let ls = definitions
            .iter()
            .find(|definition| definition.name == "ls")
            .expect("ls definition");
        let grep = definitions
            .iter()
            .find(|definition| definition.name == "grep")
            .expect("grep definition");
        let write_file = definitions
            .iter()
            .find(|definition| definition.name == "write_file")
            .expect("write_file definition");

        assert!(read_file.description.contains("Use `ls` first"));
        assert!(
            read_file
                .description
                .contains("Relative paths are resolved")
        );
        assert!(read_file.description.contains("external absolute paths"));
        assert!(read_file.description.contains("locked workspace"));
        assert!(read_file.description.contains("Do not pass directories"));
        assert!(
            read_file
                .description
                .contains("not-yet-created output directories or files")
        );
        assert!(read_file.description.contains("binary sidecars"));
        assert!(read_file.description.contains("guessed files"));
        assert!(
            read_file
                .input_schema
                .to_string()
                .contains("Existing file path inside the locked workspace")
        );
        assert!(
            read_file
                .input_schema
                .to_string()
                .contains("canonical/symlink resolution stays under the locked workspace")
        );
        assert!(ls.description.contains("report one file entry"));
        assert!(ls.description.contains("Relative paths are resolved"));
        assert!(ls.description.contains("locked workspace"));
        assert!(ls.description.contains("generated output file exists"));
        assert!(
            ls.description
                .contains("Do not keep listing guessed missing output directories")
        );
        assert!(
            ls.input_schema
                .to_string()
                .contains("Existing file or directory path inside the locked workspace")
        );
        assert!(grep.description.contains("locked workspace"));
        assert!(
            grep.input_schema
                .to_string()
                .contains("inside the locked workspace")
        );
        assert!(write_file.description.contains("locked workspace"));
        let combined_schema = definitions
            .iter()
            .map(|definition| format!("{}\n{}", definition.description, definition.input_schema))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(!combined_schema.contains("absolute readable paths are allowed"));
    }

    #[test]
    fn worker_implemented_tool_surface_excludes_shell_and_recursive_task() {
        let registry = BuiltinToolRegistry::reasonix_aligned();
        let names = registry
            .worker_implemented_definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();

        assert_eq!(
            names,
            vec![
                "camo".to_owned(),
                "complete_step".to_owned(),
                "delete_range".to_owned(),
                "edit_file".to_owned(),
                "glob".to_owned(),
                "grep".to_owned(),
                "ls".to_owned(),
                "multi_edit".to_owned(),
                "read_file".to_owned(),
                "todo_write".to_owned(),
                "web_fetch".to_owned(),
                "write_file".to_owned(),
            ],
            "worker-visible tools must be exact so the model does not guess unavailable names"
        );
        assert!(!names.contains(&"bash".to_owned()));
        assert!(!names.contains(&"shell".to_owned()));
        assert!(!names.contains(&"readlink".to_owned()));
        assert!(!names.contains(&"pwd".to_owned()));
        assert!(!names.contains(&"cat".to_owned()));
        assert!(!names.contains(&"find".to_owned()));
        assert!(names.contains(&"read_file".to_owned()));
        assert!(names.contains(&"write_file".to_owned()));
        assert!(names.contains(&"todo_write".to_owned()));
        assert!(!names.contains(&"task".to_owned()));
        assert!(!names.contains(&"timer".to_owned()));
        assert!(!registry.worker_implemented_schema_fingerprint().is_empty());
    }

    #[test]
    fn locked_workspace_root_accepts_configured_daemon_workspace() {
        let root = env::temp_dir().join(format!(
            "freehand-tools-configured-root-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("create configured root");

        let resolved =
            locked_workspace_root_from_env("write_file", Some(root.clone().into_os_string()))
                .expect("configured root");

        assert_eq!(resolved, fs::canonicalize(&root).expect("canonical root"));
        fs::remove_dir_all(root).expect("cleanup configured root");
    }

    #[test]
    fn implemented_schema_fingerprint_is_stable_across_registration_order() {
        let canonical = BuiltinToolRegistry::reasonix_aligned();
        let mut reversed = BuiltinToolRegistry {
            tools: BTreeMap::new(),
        };
        let mut specs = reasonix_aligned_builtin_specs();
        specs.reverse();
        for spec in specs {
            reversed.register(spec);
        }

        assert_eq!(
            canonical.implemented_schema_fingerprint(),
            reversed.implemented_schema_fingerprint()
        );
    }

    #[test]
    fn implemented_schema_fingerprint_changes_when_implemented_schema_changes() {
        let baseline = BuiltinToolRegistry::reasonix_aligned();
        let mut changed = baseline.clone();
        changed.register(spec(
            "alpha_tool",
            true,
            true,
            "Added implemented schema for planner diagnostics drift coverage.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "recursive": {"type": "boolean"}
                },
                "required": ["path"]
            }),
        ));

        assert_ne!(
            baseline.implemented_schema_fingerprint(),
            changed.implemented_schema_fingerprint()
        );
    }

    #[test]
    fn todo_write_executes() {
        let registry = BuiltinToolRegistry::reasonix_aligned();
        let call = tool_call(
            "todo_write",
            vec![ToolArgument {
                name: "todos".to_owned(),
                value: json!([
                    {"content": "Check UI", "status": "completed"},
                    {"content": "Run tests", "status": "in_progress"}
                ]),
            }],
        );
        let output = registry.execute(&call).expect("todo executes");
        assert!(output.text.contains("2 total"));
    }

    #[test]
    fn delete_range_preview_execute_parity() {
        with_temp_workspace(|test_root| {
            let target = test_root.join("note.txt");
            // Use a file with unique single-line anchors (no newline in anchors)
            fs::write(
                &target,
                "StartLine
MiddleContent
EndLine
",
            )
            .expect("seed file");

            let registry = BuiltinToolRegistry::reasonix_aligned();
            // inclusive=true: delete from start anchor to end anchor inclusive
            let tool_call = tool_call(
                "delete_range",
                vec![
                    ToolArgument {
                        name: "path".to_owned(),
                        value: json!("note.txt"),
                    },
                    ToolArgument {
                        name: "start_anchor".to_owned(),
                        value: json!("StartLine"),
                    },
                    ToolArgument {
                        name: "end_anchor".to_owned(),
                        value: json!("EndLine"),
                    },
                    ToolArgument {
                        name: "inclusive".to_owned(),
                        value: json!(true),
                    },
                ],
            );
            let preview = registry.preview(&tool_call).expect("preview");
            assert_eq!(preview.changes.len(), 1);
            let change = &preview.changes[0];
            assert_eq!(change.kind, ToolPreviewChangeKind::Delete);
            // inclusive=true removes anchors too: everything from "StartLine" through "EndLine" inclusive
            assert_eq!(
                change.after_text.as_deref(),
                Some(
                    "
"
                )
            );
            assert_eq!(
                change.before_text.as_deref(),
                Some(
                    "StartLine
MiddleContent
EndLine
"
                )
            );

            let output = registry.execute(&tool_call).expect("execute");
            assert!(output.text.contains("Deleted range"));
            let persisted = fs::read_to_string(&target).expect("read after execute");
            assert_eq!(
                persisted,
                "
"
            );
        });
    }

    #[test]
    fn delete_range_preview_rejects_missing_anchors() {
        with_temp_workspace(|test_root| {
            let target = test_root.join("note.txt");
            fs::write(
                &target,
                "alpha
beta
",
            )
            .expect("seed file");

            let registry = BuiltinToolRegistry::reasonix_aligned();
            let tool_call = tool_call(
                "delete_range",
                vec![
                    ToolArgument {
                        name: "path".to_owned(),
                        value: json!("note.txt"),
                    },
                    ToolArgument {
                        name: "start_anchor".to_owned(),
                        value: json!("missing"),
                    },
                    ToolArgument {
                        name: "end_anchor".to_owned(),
                        value: json!("also-missing"),
                    },
                ],
            );
            let err = registry
                .preview(&tool_call)
                .expect_err("preview must reject missing anchors");
            assert!(matches!(err, ToolRegistryError::ExecutionFailed { .. }));
        });
    }
    #[test]
    fn bash_runs_in_workspace_root_and_returns_output() {
        with_temp_workspace(|root| {
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let canonical_root = fs::canonicalize(root).expect("canonical root");
            let output = registry
                .execute(&tool_call(
                    "bash",
                    vec![ToolArgument {
                        name: "command".to_owned(),
                        value: json!("pwd"),
                    }],
                ))
                .expect("bash executes");
            assert_eq!(output.text.trim(), canonical_root.to_string_lossy());
        });
    }

    #[test]
    fn bash_reports_non_zero_exit_with_captured_output() {
        with_temp_workspace(|_| {
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let result = registry.execute(&tool_call(
                "bash",
                vec![ToolArgument {
                    name: "command".to_owned(),
                    value: json!("echo boom 1>&2; exit 7"),
                }],
            ));
            assert!(matches!(
                result,
                Err(ToolRegistryError::ExecutionFailed { tool, message })
                    if tool == "bash"
                        && message.contains("command exited with status")
                        && message.contains("boom")
            ));
        });
    }

    #[test]
    fn bash_times_out_explicitly() {
        with_temp_workspace(|_| {
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let result = registry.execute(&tool_call(
                "bash",
                vec![
                    ToolArgument {
                        name: "command".to_owned(),
                        value: json!("sleep 2"),
                    },
                    ToolArgument {
                        name: "timeout_seconds".to_owned(),
                        value: json!(1),
                    },
                ],
            ));
            assert!(matches!(
                result,
                Err(ToolRegistryError::ExecutionFailed { tool, message })
                    if tool == "bash" && message.contains("timed out after 1 second")
            ));
        });
    }

    #[test]
    fn bash_rejects_zero_timeout() {
        with_temp_workspace(|_| {
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let result = registry.execute(&tool_call(
                "bash",
                vec![
                    ToolArgument {
                        name: "command".to_owned(),
                        value: json!("pwd"),
                    },
                    ToolArgument {
                        name: "timeout_seconds".to_owned(),
                        value: json!(0),
                    },
                ],
            ));
            assert_eq!(
                result,
                Err(ToolRegistryError::InvalidArguments {
                    tool: "bash".to_owned(),
                    message: "`timeout_seconds` must be at least 1".to_owned(),
                })
            );
        });
    }

    #[test]
    fn read_file_reads_window_and_reports_more_lines() {
        with_temp_workspace(|root| {
            fs::write(root.join("notes.txt"), "one\ntwo\nthree\n").expect("write notes");
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let output = registry
                .execute(&tool_call(
                    "read_file",
                    vec![
                        ToolArgument {
                            name: "path".to_owned(),
                            value: json!("notes.txt"),
                        },
                        ToolArgument {
                            name: "offset".to_owned(),
                            value: json!(1),
                        },
                        ToolArgument {
                            name: "limit".to_owned(),
                            value: json!(1),
                        },
                    ],
                ))
                .expect("read_file executes");
            assert!(output.text.contains("notes.txt"));
            assert!(output.text.contains("2|two"));
            assert!(output.text.contains("pass offset=2"));
        });
    }

    #[test]
    fn write_file_creates_and_overwrites_files_inside_workspace() {
        with_temp_workspace(|root| {
            fs::create_dir_all(root.join("docs")).expect("create docs");
            let registry = BuiltinToolRegistry::reasonix_aligned();

            let created = registry
                .execute(&tool_call(
                    "write_file",
                    vec![
                        ToolArgument {
                            name: "path".to_owned(),
                            value: json!("docs/new.txt"),
                        },
                        ToolArgument {
                            name: "content".to_owned(),
                            value: json!("hello"),
                        },
                    ],
                ))
                .expect("write_file creates");
            assert!(created.text.contains("Created `docs/new.txt`"));
            assert_eq!(
                fs::read_to_string(root.join("docs/new.txt")).expect("read created"),
                "hello"
            );

            let overwritten = registry
                .execute(&tool_call(
                    "write_file",
                    vec![
                        ToolArgument {
                            name: "path".to_owned(),
                            value: json!("docs/new.txt"),
                        },
                        ToolArgument {
                            name: "content".to_owned(),
                            value: json!("updated"),
                        },
                    ],
                ))
                .expect("write_file overwrites");
            assert!(overwritten.text.contains("Overwrote `docs/new.txt`"));
            assert_eq!(
                fs::read_to_string(root.join("docs/new.txt")).expect("read overwritten"),
                "updated"
            );
        });
    }

    #[test]
    fn write_file_preview_matches_execute_for_create_and_overwrite() {
        with_temp_workspace(|root| {
            fs::create_dir_all(root.join("docs")).expect("create docs");
            fs::write(root.join("docs/existing.txt"), "old").expect("seed existing");
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let canonical_root = fs::canonicalize(root).expect("canonical root");

            let create_call = tool_call(
                "write_file",
                vec![
                    ToolArgument {
                        name: "path".to_owned(),
                        value: json!("docs/new.txt"),
                    },
                    ToolArgument {
                        name: "content".to_owned(),
                        value: json!("hello"),
                    },
                ],
            );
            let create_preview = registry.preview(&create_call).expect("create preview");
            assert_eq!(
                create_preview.changes,
                vec![ToolPreviewFileChange {
                    locked_path: canonical_root
                        .join("docs/new.txt")
                        .to_string_lossy()
                        .into_owned(),
                    kind: ToolPreviewChangeKind::Create,
                    before_text: None,
                    after_text: Some("hello".to_owned()),
                }]
            );
            registry.execute(&create_call).expect("create executes");
            assert_eq!(
                fs::read_to_string(root.join("docs/new.txt")).expect("read new"),
                create_preview.changes[0]
                    .after_text
                    .as_deref()
                    .expect("create after text")
            );

            let overwrite_call = tool_call(
                "write_file",
                vec![
                    ToolArgument {
                        name: "path".to_owned(),
                        value: json!("docs/existing.txt"),
                    },
                    ToolArgument {
                        name: "content".to_owned(),
                        value: json!("updated"),
                    },
                ],
            );
            let overwrite_preview = registry
                .preview(&overwrite_call)
                .expect("overwrite preview");
            assert_eq!(
                overwrite_preview.changes,
                vec![ToolPreviewFileChange {
                    locked_path: canonical_root
                        .join("docs/existing.txt")
                        .to_string_lossy()
                        .into_owned(),
                    kind: ToolPreviewChangeKind::Modify,
                    before_text: Some("old".to_owned()),
                    after_text: Some("updated".to_owned()),
                }]
            );
            registry
                .execute(&overwrite_call)
                .expect("overwrite executes");
            assert_eq!(
                fs::read_to_string(root.join("docs/existing.txt")).expect("read overwritten"),
                overwrite_preview.changes[0]
                    .after_text
                    .as_deref()
                    .expect("overwrite after text")
            );
        });
    }

    #[test]
    fn write_file_rejects_escape_and_missing_parent() {
        with_temp_workspace(|root| {
            let parent = root.parent().expect("parent");
            fs::write(parent.join("outside-write.txt"), "secret").expect("write outside");
            let registry = BuiltinToolRegistry::reasonix_aligned();

            let escape = registry.execute(&tool_call(
                "write_file",
                vec![
                    ToolArgument {
                        name: "path".to_owned(),
                        value: json!("../outside-write.txt"),
                    },
                    ToolArgument {
                        name: "content".to_owned(),
                        value: json!("replace"),
                    },
                ],
            ));
            assert!(matches!(
                escape,
                Err(ToolRegistryError::WorkspaceBoundaryViolation {
                    tool,
                    field,
                    ..
                }) if tool == "write_file" && field == "path"
            ));

            let missing_parent = registry.execute(&tool_call(
                "write_file",
                vec![
                    ToolArgument {
                        name: "path".to_owned(),
                        value: json!("missing/new.txt"),
                    },
                    ToolArgument {
                        name: "content".to_owned(),
                        value: json!("replace"),
                    },
                ],
            ));
            assert!(matches!(
                missing_parent,
                Err(ToolRegistryError::ExecutionFailed { tool, .. }) if tool == "write_file"
            ));
        });
    }

    #[test]
    fn edit_file_replaces_exact_single_occurrence() {
        with_temp_workspace(|root| {
            fs::write(root.join("notes.txt"), "alpha\nbeta\n").expect("write notes");
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let output = registry
                .execute(&tool_call(
                    "edit_file",
                    vec![
                        ToolArgument {
                            name: "path".to_owned(),
                            value: json!("notes.txt"),
                        },
                        ToolArgument {
                            name: "old_string".to_owned(),
                            value: json!("beta"),
                        },
                        ToolArgument {
                            name: "new_string".to_owned(),
                            value: json!("gamma"),
                        },
                    ],
                ))
                .expect("edit file");
            assert!(output.text.contains("replacing 1 exact occurrence"));
            assert_eq!(
                fs::read_to_string(root.join("notes.txt")).expect("read edited"),
                "alpha\ngamma\n"
            );
        });
    }

    #[test]
    fn edit_file_preview_matches_execute() {
        with_temp_workspace(|root| {
            fs::write(root.join("notes.txt"), "alpha\nbeta\n").expect("write notes");
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let canonical_root = fs::canonicalize(root).expect("canonical root");
            let call = tool_call(
                "edit_file",
                vec![
                    ToolArgument {
                        name: "path".to_owned(),
                        value: json!("notes.txt"),
                    },
                    ToolArgument {
                        name: "old_string".to_owned(),
                        value: json!("beta"),
                    },
                    ToolArgument {
                        name: "new_string".to_owned(),
                        value: json!("gamma"),
                    },
                ],
            );

            let preview = registry.preview(&call).expect("edit preview");
            assert_eq!(
                preview.changes,
                vec![ToolPreviewFileChange {
                    locked_path: canonical_root
                        .join("notes.txt")
                        .to_string_lossy()
                        .into_owned(),
                    kind: ToolPreviewChangeKind::Modify,
                    before_text: Some("alpha\nbeta\n".to_owned()),
                    after_text: Some("alpha\ngamma\n".to_owned()),
                }]
            );

            registry.execute(&call).expect("edit execute");
            assert_eq!(
                fs::read_to_string(root.join("notes.txt")).expect("read edited"),
                preview.changes[0]
                    .after_text
                    .as_deref()
                    .expect("edit after text")
            );
        });
    }

    #[test]
    fn edit_file_rejects_zero_or_multiple_matches() {
        with_temp_workspace(|root| {
            fs::write(root.join("notes.txt"), "beta\nbeta\n").expect("write notes");
            let registry = BuiltinToolRegistry::reasonix_aligned();

            let multiple = registry.execute(&tool_call(
                "edit_file",
                vec![
                    ToolArgument {
                        name: "path".to_owned(),
                        value: json!("notes.txt"),
                    },
                    ToolArgument {
                        name: "old_string".to_owned(),
                        value: json!("beta"),
                    },
                    ToolArgument {
                        name: "new_string".to_owned(),
                        value: json!("gamma"),
                    },
                ],
            ));
            assert!(matches!(
                multiple,
                Err(ToolRegistryError::ExecutionFailed { tool, .. }) if tool == "edit_file"
            ));

            let missing = registry.execute(&tool_call(
                "edit_file",
                vec![
                    ToolArgument {
                        name: "path".to_owned(),
                        value: json!("notes.txt"),
                    },
                    ToolArgument {
                        name: "old_string".to_owned(),
                        value: json!("absent"),
                    },
                    ToolArgument {
                        name: "new_string".to_owned(),
                        value: json!("gamma"),
                    },
                ],
            ));
            assert!(matches!(
                missing,
                Err(ToolRegistryError::ExecutionFailed { tool, .. }) if tool == "edit_file"
            ));
        });
    }

    #[test]
    fn multi_edit_applies_sequential_and_replace_all_edits() {
        with_temp_workspace(|root| {
            fs::write(root.join("notes.txt"), "alpha\nbeta\nbeta\n").expect("write notes");
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let output = registry
                .execute(&tool_call(
                    "multi_edit",
                    vec![
                        ToolArgument {
                            name: "path".to_owned(),
                            value: json!("notes.txt"),
                        },
                        ToolArgument {
                            name: "edits".to_owned(),
                            value: json!([
                                {
                                    "old_string": "alpha",
                                    "new_string": "start"
                                },
                                {
                                    "old_string": "beta",
                                    "new_string": "done",
                                    "replace_all": true
                                }
                            ]),
                        },
                    ],
                ))
                .expect("multi edit");
            assert!(output.text.contains("2 edit(s)"));
            assert!(output.text.contains("3 replacement(s)"));
            assert_eq!(
                fs::read_to_string(root.join("notes.txt")).expect("read multi edited"),
                "start\ndone\ndone\n"
            );
        });
    }

    #[test]
    fn multi_edit_preview_matches_execute() {
        with_temp_workspace(|root| {
            fs::write(root.join("notes.txt"), "alpha\nbeta\nbeta\n").expect("write notes");
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let canonical_root = fs::canonicalize(root).expect("canonical root");
            let call = tool_call(
                "multi_edit",
                vec![
                    ToolArgument {
                        name: "path".to_owned(),
                        value: json!("notes.txt"),
                    },
                    ToolArgument {
                        name: "edits".to_owned(),
                        value: json!([
                            {
                                "old_string": "alpha",
                                "new_string": "start"
                            },
                            {
                                "old_string": "beta",
                                "new_string": "done",
                                "replace_all": true
                            }
                        ]),
                    },
                ],
            );

            let preview = registry.preview(&call).expect("multi-edit preview");
            assert_eq!(
                preview.changes,
                vec![ToolPreviewFileChange {
                    locked_path: canonical_root
                        .join("notes.txt")
                        .to_string_lossy()
                        .into_owned(),
                    kind: ToolPreviewChangeKind::Modify,
                    before_text: Some("alpha\nbeta\nbeta\n".to_owned()),
                    after_text: Some("start\ndone\ndone\n".to_owned()),
                }]
            );

            registry.execute(&call).expect("multi-edit execute");
            assert_eq!(
                fs::read_to_string(root.join("notes.txt")).expect("read multi edited"),
                preview.changes[0]
                    .after_text
                    .as_deref()
                    .expect("multi-edit after text")
            );
        });
    }

    #[test]
    fn multi_edit_rejects_missing_target_text() {
        with_temp_workspace(|root| {
            fs::write(root.join("notes.txt"), "alpha\n").expect("write notes");
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let result = registry.execute(&tool_call(
                "multi_edit",
                vec![
                    ToolArgument {
                        name: "path".to_owned(),
                        value: json!("notes.txt"),
                    },
                    ToolArgument {
                        name: "edits".to_owned(),
                        value: json!([
                            {
                                "old_string": "beta",
                                "new_string": "gamma",
                                "replace_all": true
                            }
                        ]),
                    },
                ],
            ));
            assert!(matches!(
                result,
                Err(ToolRegistryError::InvalidArguments { tool, .. }) if tool == "multi_edit"
            ));
        });
    }

    #[test]
    fn read_file_rejects_parent_escape_outside_workspace_root() {
        with_temp_workspace(|root| {
            let parent = root.parent().expect("parent");
            let outside = parent.join("outside.txt");
            fs::write(&outside, "outside-readable\n").expect("write outside");
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let output = registry.execute(&tool_call(
                "read_file",
                vec![ToolArgument {
                    name: "path".to_owned(),
                    value: json!("../outside.txt"),
                }],
            ));
            assert!(matches!(
                output,
                Err(ToolRegistryError::WorkspaceBoundaryViolation { tool, field, .. })
                    if tool == "read_file" && field == "path"
            ));
            fs::remove_file(outside).expect("cleanup outside file");
        });
    }

    #[test]
    fn missing_relative_path_error_reports_absolute_workspace_diagnostic() {
        with_temp_workspace(|root| {
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let result = registry.execute(&tool_call(
                "ls",
                vec![ToolArgument {
                    name: "path".to_owned(),
                    value: json!("missing/codex"),
                }],
            ));

            let Err(ToolRegistryError::ExecutionFailed { tool, message }) = result else {
                panic!("expected ls execution failure with path diagnostic");
            };
            let canonical_root = fs::canonicalize(root).expect("canonical root");
            assert_eq!(tool, "ls");
            assert!(message.contains("path_diagnostic"));
            assert!(message.contains("requested=`missing/codex`"));
            assert!(message.contains(&format!("locked_workspace=`{}`", canonical_root.display())));
            assert!(message.contains(&format!(
                "absolute=`{}`",
                canonical_root.join("missing/codex").display()
            )));
            assert!(message.contains(&format!("nearest_existing=`{}`", canonical_root.display())));
            assert!(message.contains("missing_suffix=`missing/codex`"));
        });
    }

    #[test]
    fn glob_matches_nested_files_and_simple_filename_patterns() {
        with_temp_workspace(|root| {
            fs::create_dir_all(root.join("src/nested")).expect("create nested");
            fs::write(root.join("src/nested/lib.rs"), "fn main() {}\n").expect("write lib");
            fs::write(root.join("README.md"), "# hi\n").expect("write readme");
            let registry = BuiltinToolRegistry::reasonix_aligned();

            let recursive = registry
                .execute(&tool_call(
                    "glob",
                    vec![ToolArgument {
                        name: "pattern".to_owned(),
                        value: json!("**/*.rs"),
                    }],
                ))
                .expect("recursive glob");
            assert!(recursive.text.contains("src/nested/lib.rs"));

            let filename_only = registry
                .execute(&tool_call(
                    "glob",
                    vec![ToolArgument {
                        name: "pattern".to_owned(),
                        value: json!("*.rs"),
                    }],
                ))
                .expect("basename glob");
            assert!(filename_only.text.contains("src/nested/lib.rs"));
        });
    }

    #[test]
    fn glob_accepts_absolute_patterns_inside_locked_workspace_only() {
        with_temp_workspace(|root| {
            fs::create_dir_all(root.join("main/audio")).expect("create audio");
            fs::write(root.join("main/audio/codec.cc"), "codec\n").expect("write codec");
            let registry = BuiltinToolRegistry::reasonix_aligned();

            let in_workspace_absolute = registry
                .execute(&tool_call(
                    "glob",
                    vec![ToolArgument {
                        name: "pattern".to_owned(),
                        value: json!(root.join("main/**/*.cc").to_string_lossy().to_string()),
                    }],
                ))
                .expect("absolute in-workspace glob executes");
            assert!(in_workspace_absolute.text.contains("main/audio/codec.cc"));

            let outside = registry.execute(&tool_call(
                "glob",
                vec![ToolArgument {
                    name: "pattern".to_owned(),
                    value: json!("/tmp/**/*.cc"),
                }],
            ));
            assert!(matches!(
                outside,
                Err(ToolRegistryError::WorkspaceBoundaryViolation { tool, field, .. })
                    if tool == "glob" && field == "pattern"
            ));
        });
    }

    #[test]
    fn read_path_tools_reject_existing_absolute_paths_outside_locked_workspace() {
        with_temp_workspace(|root| {
            let outside = root.parent().expect("temp parent").join(format!(
                "{}-outside",
                root.file_name().unwrap().to_string_lossy()
            ));
            fs::create_dir_all(&outside).expect("create outside directory");
            let outside_file = outside.join("outside.txt");
            fs::write(&outside_file, "outside\n").expect("write outside file");
            let registry = BuiltinToolRegistry::reasonix_aligned();

            let read = registry.execute(&tool_call(
                "read_file",
                vec![ToolArgument {
                    name: "path".to_owned(),
                    value: json!(outside_file.to_string_lossy().to_string()),
                }],
            ));
            assert!(matches!(
                read,
                Err(ToolRegistryError::WorkspaceBoundaryViolation { tool, field, .. })
                    if tool == "read_file" && field == "path"
            ));

            let list = registry.execute(&tool_call(
                "ls",
                vec![ToolArgument {
                    name: "path".to_owned(),
                    value: json!(outside.to_string_lossy().to_string()),
                }],
            ));
            assert!(matches!(
                list,
                Err(ToolRegistryError::WorkspaceBoundaryViolation { tool, field, .. })
                    if tool == "ls" && field == "path"
            ));

            let search = registry.execute(&tool_call(
                "grep",
                vec![
                    ToolArgument {
                        name: "pattern".to_owned(),
                        value: json!("outside"),
                    },
                    ToolArgument {
                        name: "path".to_owned(),
                        value: json!(outside.to_string_lossy().to_string()),
                    },
                ],
            ));
            assert!(matches!(
                search,
                Err(ToolRegistryError::WorkspaceBoundaryViolation { tool, field, .. })
                    if tool == "grep" && field == "path"
            ));

            fs::remove_dir_all(outside).expect("cleanup outside directory");
        });
    }

    #[test]
    fn path_tools_accept_absolute_symlink_aliases_inside_locked_workspace() {
        with_temp_workspace(|root| {
            fs::create_dir_all(root.join("main/boards")).expect("create boards");
            fs::write(root.join("main/boards/board.cc"), "board\n").expect("write board");
            let alias = root.parent().expect("temp parent").join(format!(
                "{}-alias",
                root.file_name().unwrap().to_string_lossy()
            ));
            std::os::unix::fs::symlink(root, &alias).expect("create workspace symlink alias");

            let registry = BuiltinToolRegistry::reasonix_aligned();
            let alias_glob = alias.join("main/**/*.cc").to_string_lossy().to_string();
            let glob_output = registry
                .execute(&tool_call(
                    "glob",
                    vec![ToolArgument {
                        name: "pattern".to_owned(),
                        value: json!(alias_glob),
                    }],
                ))
                .expect("glob follows in-workspace symlink alias");
            assert!(glob_output.text.contains("main/boards/board.cc"));

            let alias_file = alias
                .join("main/boards/board.cc")
                .to_string_lossy()
                .to_string();
            let read_output = registry
                .execute(&tool_call(
                    "read_file",
                    vec![ToolArgument {
                        name: "path".to_owned(),
                        value: json!(alias_file),
                    }],
                ))
                .expect("read_file follows in-workspace symlink alias");
            assert!(read_output.text.contains("1|board"));

            let alias_dir = alias.join("main/boards").to_string_lossy().to_string();
            let ls_output = registry
                .execute(&tool_call(
                    "ls",
                    vec![ToolArgument {
                        name: "path".to_owned(),
                        value: json!(alias_dir),
                    }],
                ))
                .expect("ls follows in-workspace symlink alias");
            assert!(ls_output.text.contains("board.cc"));

            let grep_output = registry
                .execute(&tool_call(
                    "grep",
                    vec![
                        ToolArgument {
                            name: "pattern".to_owned(),
                            value: json!("board"),
                        },
                        ToolArgument {
                            name: "path".to_owned(),
                            value: json!(alias.join("main").to_string_lossy().to_string()),
                        },
                    ],
                ))
                .expect("grep follows in-workspace symlink alias");
            assert!(grep_output.text.contains("main/boards/board.cc:1:board"));

            let alias_write = alias
                .join("main/boards/generated.txt")
                .to_string_lossy()
                .to_string();
            let write_output = registry
                .execute(&tool_call(
                    "write_file",
                    vec![
                        ToolArgument {
                            name: "path".to_owned(),
                            value: json!(alias_write),
                        },
                        ToolArgument {
                            name: "content".to_owned(),
                            value: json!("generated\n"),
                        },
                    ],
                ))
                .expect("write_file accepts in-workspace symlink alias");
            assert!(write_output.text.contains("generated.txt"));
            assert_eq!(
                fs::read_to_string(root.join("main/boards/generated.txt")).expect("read written"),
                "generated\n"
            );

            fs::remove_file(alias).expect("cleanup symlink alias");
        });
    }

    #[test]
    fn path_tools_report_symlink_parent_when_absolute_leaf_is_missing() {
        with_temp_workspace(|root| {
            let real_parent = root.join("Documents").join("github");
            fs::create_dir_all(&real_parent).expect("create real parent");
            let alias_parent = root.join("github");
            std::os::unix::fs::symlink(&real_parent, &alias_parent).expect("create parent symlink");
            let requested = alias_parent.join("codex");
            let registry = BuiltinToolRegistry::reasonix_aligned();

            let result = registry.execute(&tool_call(
                "ls",
                vec![ToolArgument {
                    name: "path".to_owned(),
                    value: json!(requested.to_string_lossy().to_string()),
                }],
            ));
            let Err(ToolRegistryError::ExecutionFailed { tool, message }) = result else {
                panic!("expected missing leaf diagnostic");
            };
            assert_eq!(tool, "ls");
            assert!(message.contains("path_diagnostic"));
            assert!(message.contains(&format!("requested=`{}`", requested.display())));
            assert!(message.contains(&format!("absolute=`{}`", requested.display())));
            assert!(message.contains(&format!("nearest_existing=`{}`", alias_parent.display())));
            assert!(message.contains(&format!(
                "nearest_existing_canonical=`{}`",
                fs::canonicalize(&real_parent)
                    .expect("canonical real parent")
                    .display()
            )));
            assert!(message.contains("missing_suffix=`codex`"));
            assert!(message.contains(&format!(
                "`{}` -> `{}`",
                alias_parent.display(),
                real_parent.display()
            )));

            let glob_result = registry
                .execute(&tool_call(
                    "glob",
                    vec![ToolArgument {
                        name: "pattern".to_owned(),
                        value: json!(
                            alias_parent
                                .join("codex/**/*.rs")
                                .to_string_lossy()
                                .to_string()
                        ),
                    }],
                ))
                .expect("glob canonicalizes symlink parent before workspace boundary");
            assert_eq!(glob_result.text, "(no matches)");
        });
    }

    #[test]
    fn grep_searches_recursive_tree() {
        with_temp_workspace(|root| {
            fs::create_dir_all(root.join("src")).expect("create src");
            fs::write(
                root.join("src/lib.rs"),
                "pub fn alpha() {}\npub fn beta() {}\n",
            )
            .expect("write lib");
            fs::write(root.join("README.md"), "alpha beta gamma\n").expect("write readme");
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let output = registry
                .execute(&tool_call(
                    "grep",
                    vec![ToolArgument {
                        name: "pattern".to_owned(),
                        value: json!("alpha"),
                    }],
                ))
                .expect("grep executes");
            assert!(output.text.contains("README.md:1:alpha beta gamma"));
            assert!(output.text.contains("src/lib.rs:1:pub fn alpha() {}"));
        });
    }

    #[test]
    fn grep_rejects_absolute_path_outside_workspace_root() {
        with_temp_workspace(|root| {
            let parent = root.parent().expect("parent");
            let outside = parent.join("outside-grep.txt");
            fs::write(&outside, "needle outside\n").expect("write outside");
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let output = registry.execute(&tool_call(
                "grep",
                vec![
                    ToolArgument {
                        name: "pattern".to_owned(),
                        value: json!("needle"),
                    },
                    ToolArgument {
                        name: "path".to_owned(),
                        value: json!(outside.to_string_lossy().to_string()),
                    },
                ],
            ));
            assert!(matches!(
                output,
                Err(ToolRegistryError::WorkspaceBoundaryViolation { tool, field, .. })
                    if tool == "grep" && field == "path"
            ));
            fs::remove_file(outside).expect("cleanup outside file");
        });
    }

    #[test]
    fn ls_lists_entries_and_recursive_tree() {
        with_temp_workspace(|root| {
            fs::create_dir_all(root.join("docs/specs")).expect("create docs");
            fs::write(root.join("docs/specs/tool.md"), "tool\n").expect("write tool");
            fs::create_dir_all(root.join("target/debug")).expect("create target");
            fs::write(root.join("target/debug/skip.me"), "skip\n").expect("write skip");
            let registry = BuiltinToolRegistry::reasonix_aligned();

            let flat = registry
                .execute(&tool_call(
                    "ls",
                    vec![ToolArgument {
                        name: "path".to_owned(),
                        value: json!("docs"),
                    }],
                ))
                .expect("ls executes");
            assert!(flat.text.contains("specs/"));

            let recursive = registry
                .execute(&tool_call(
                    "ls",
                    vec![
                        ToolArgument {
                            name: "path".to_owned(),
                            value: json!("."),
                        },
                        ToolArgument {
                            name: "recursive".to_owned(),
                            value: json!(true),
                        },
                    ],
                ))
                .expect("ls recursive executes");
            assert!(recursive.text.contains("docs/specs/"));
            assert!(recursive.text.contains("docs/specs/tool.md"));
            assert!(!recursive.text.contains("target/debug/skip.me"));

            let file_entry = registry
                .execute(&tool_call(
                    "ls",
                    vec![ToolArgument {
                        name: "path".to_owned(),
                        value: json!("docs/specs/tool.md"),
                    }],
                ))
                .expect("ls reports file entry");
            assert!(file_entry.text.contains("docs/specs/tool.md\t5"));
        });
    }

    #[test]
    fn ls_rejects_absolute_path_outside_workspace_root() {
        with_temp_workspace(|root| {
            let parent = root.parent().expect("parent");
            let outside_dir = parent.join("outside-list");
            fs::create_dir_all(&outside_dir).expect("create outside dir");
            fs::write(outside_dir.join("visible.txt"), "visible\n").expect("write outside file");
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let output = registry.execute(&tool_call(
                "ls",
                vec![ToolArgument {
                    name: "path".to_owned(),
                    value: json!(outside_dir.to_string_lossy().to_string()),
                }],
            ));
            assert!(matches!(
                output,
                Err(ToolRegistryError::WorkspaceBoundaryViolation { tool, field, .. })
                    if tool == "ls" && field == "path"
            ));
            fs::remove_dir_all(outside_dir).expect("cleanup outside directory");
        });
    }

    #[test]
    fn unknown_and_unimplemented_tools_fail_explicitly() {
        let registry = BuiltinToolRegistry::reasonix_aligned();
        assert_eq!(
            registry.execute(&tool_call("missing_tool", vec![])),
            Err(ToolRegistryError::UnknownTool("missing_tool".to_owned()))
        );
        assert!(matches!(
            registry.execute(&tool_call("web_fetch", vec![])),
            Err(ToolRegistryError::InvalidArguments { tool, .. }) if tool == "web_fetch"
        ));
        assert_eq!(
            registry.execute(&tool_call(
                "web_fetch",
                vec![ToolArgument {
                    name: "url".to_owned(),
                    value: json!("ftp://example.com"),
                }],
            )),
            Err(ToolRegistryError::InvalidArguments {
                tool: "web_fetch".to_owned(),
                message: "`url` must start with http:// or https://".to_owned(),
            })
        );
    }

    #[test]
    fn web_fetch_executes_against_local_http_url() {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local web fixture");
        let addr = listener.local_addr().expect("local addr");
        let handle = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept");
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request).expect("read request");
            let body = "freehand web fetch fixture";
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: text/plain; charset=utf-8\r\ncontent-length: {}\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });
        let registry = BuiltinToolRegistry::reasonix_aligned();
        let output = registry
            .execute(&tool_call(
                "web_fetch",
                vec![ToolArgument {
                    name: "url".to_owned(),
                    value: json!(format!("http://{addr}/fixture")),
                }],
            ))
            .expect("web fetch succeeds");
        handle.join().expect("fixture server joins");
        assert!(output.text.contains("status=200"));
        assert!(output.text.contains("text/plain"));
        assert!(output.text.contains("freehand web fetch fixture"));
    }

    #[test]
    fn task_management_semantic_actions_are_not_exposed_as_tools() {
        let registry = BuiltinToolRegistry::reasonix_aligned();
        let exposed_names = registry
            .definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();
        let semantic_action_names = [
            "query_task_board",
            "query_agent_board",
            "query_agent_tasks",
            "query_blocked_tasks",
            "query_review_queue",
            "query_stale_executions",
            "create_subtask",
            "dispatch_subtask",
            "query_execution",
            "query_agent_lifecycle",
            "ask_runtime_question",
            "inject_constraint",
            "approve_submission",
            "reject_submission",
            "wait_with_next_check",
            "close_big_task",
        ];

        assert!(exposed_names.contains(&"task".to_owned()));
        for semantic_name in semantic_action_names {
            assert!(
                !exposed_names.contains(&semantic_name.to_owned()),
                "`{semantic_name}` is a semantic action category and must not be exposed as a standalone tool"
            );
        }
    }

    #[test]
    fn task_tool_exposes_operation_parameter() {
        let task_definition = reasonix_aligned_builtin_specs()
            .into_iter()
            .find(|spec| spec.definition.name == "task")
            .expect("task tool spec")
            .definition;
        let required = task_definition
            .input_schema
            .get("required")
            .and_then(serde_json::Value::as_array)
            .expect("required array");
        let op_schema = task_definition
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .and_then(|properties| properties.get("op"))
            .expect("op schema");
        let properties = task_definition
            .input_schema
            .get("properties")
            .and_then(serde_json::Value::as_object)
            .expect("task properties");
        let status_schema = properties.get("status").expect("status schema");
        let execution_id_schema = properties.get("execution_id").expect("execution_id schema");
        let retry_count_schema = properties.get("retry_count").expect("retry_count schema");
        let dispatch_schema = properties.get("dispatch").expect("dispatch schema");
        let target_cwd_schema = properties.get("target_cwd").expect("target_cwd schema");

        assert!(task_definition.description.contains("TaskSpaceSnapshot"));
        assert!(
            task_definition
                .description
                .contains("top-level JSON object must include \"op\"")
        );
        assert!(task_definition.description.contains("\"op\":\"create\""));
        assert!(
            task_definition
                .description
                .contains("Do not call task with only a title/content payload")
        );
        assert!(required.iter().any(|item| item.as_str() == Some("op")));
        assert_eq!(
            task_definition
                .input_schema
                .get("additionalProperties")
                .and_then(serde_json::Value::as_bool),
            Some(false)
        );
        assert_eq!(
            op_schema.get("type").and_then(serde_json::Value::as_str),
            Some("string")
        );
        assert!(
            op_schema
                .get("enum")
                .and_then(serde_json::Value::as_array)
                .expect("op enum")
                .iter()
                .any(|item| item.as_str() == Some("create"))
        );
        assert!(
            op_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("op description")
                .contains("record_execution")
        );
        assert!(
            op_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("op description")
                .contains("Never omit op")
        );
        assert!(
            op_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("op description")
                .contains("Never call task with only title/content/goal")
        );
        assert!(
            op_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("op description")
                .contains("\"dispatch\":{\"mode\":\"none\"}")
        );
        assert!(
            op_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("op description")
                .contains("do not use status=\"all\"")
        );
        assert!(
            status_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("status description")
                .contains("review_ready")
        );
        assert!(
            status_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("status description")
                .contains("Omit status to list all visible tasks")
        );
        assert!(
            status_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("status description")
                .contains("do not pass all")
        );
        assert!(
            status_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("status description")
                .contains("interrupted")
        );
        assert!(
            dispatch_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("dispatch description")
                .contains("Do not use auto/self")
        );
        assert!(
            target_cwd_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("target_cwd description")
                .contains("expanded absolute path")
        );
        assert!(
            target_cwd_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("target_cwd description")
                .contains("Leading-~/symlink aliases are valid")
        );
        assert!(
            target_cwd_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("target_cwd description")
                .contains("canonical-path evidence")
        );
        assert!(
            execution_id_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("execution id description")
                .contains("record_execution")
        );
        assert_eq!(
            retry_count_schema
                .get("type")
                .and_then(serde_json::Value::as_str),
            Some("integer")
        );
        assert!(
            retry_count_schema
                .get("description")
                .and_then(serde_json::Value::as_str)
                .expect("retry count description")
                .contains("recovering")
        );
    }

    #[test]
    fn preview_rejects_non_mutation_tools_and_unimplemented_preview_tools() {
        let registry = BuiltinToolRegistry::reasonix_aligned();
        assert_eq!(
            registry.preview(&tool_call("web_fetch", vec![])),
            Err(ToolRegistryError::InvalidArguments {
                tool: "web_fetch".to_owned(),
                message: "preview is only supported for writable file-mutation tools".to_owned(),
            })
        );
        assert_eq!(
            registry.preview(&tool_call(
                "read_file",
                vec![ToolArgument {
                    name: "path".to_owned(),
                    value: json!("notes.txt"),
                }],
            )),
            Err(ToolRegistryError::InvalidArguments {
                tool: "read_file".to_owned(),
                message: "preview is only supported for writable file-mutation tools".to_owned(),
            })
        );
    }

    fn tool_call(name: &str, arguments: Vec<ToolArgument>) -> ReasonReq04ToolCall {
        ReasonReq04ToolCall {
            session_id: SessionId::new("session-1"),
            turn_id: TurnId::new("turn-1"),
            trace_id: TraceId::new("trace-1"),
            feature_id: FeatureId::new("freehand-tools"),
            agent_id: AgentId::new("agent-1"),
            tool_call: ToolCallContract {
                tool_call_id: ToolCallId::new(format!("tool-{name}")),
                tool_name: name.to_owned(),
                arguments,
                arguments_complete: true,
            },
        }
    }

    fn with_temp_workspace<F>(test: F)
    where
        F: FnOnce(&Path),
    {
        let lock = cwd_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let original = env::current_dir().expect("current dir");
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let root =
            env::temp_dir().join(format!("freehand-tools-{}-{}", std::process::id(), unique));
        fs::create_dir_all(&root).expect("create temp workspace");
        env::set_current_dir(&root).expect("set cwd");
        let restore = RestoreCwd {
            original,
            _lock: lock,
        };
        test(&root);
        drop(restore);
        fs::remove_dir_all(&root).expect("cleanup temp workspace");
    }

    fn cwd_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    struct RestoreCwd<'a> {
        original: PathBuf,
        _lock: std::sync::MutexGuard<'a, ()>,
    }

    impl Drop for RestoreCwd<'_> {
        fn drop(&mut self) {
            let _ = env::set_current_dir(&self.original);
        }
    }
}
