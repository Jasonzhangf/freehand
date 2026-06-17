//! Tool registry and built-in tool surface for Freehand.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use freehand_blocks::render_tool_arguments_json;
use freehand_contracts::{ReasonReq04ToolCall, ToolArgument};
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
}

const READ_FILE_DEFAULT_LIMIT: usize = 2_000;
const BASH_DEFAULT_TIMEOUT_SECONDS: usize = 900;
const BASH_POLL_INTERVAL_MILLIS: u64 = 20;
const GLOB_MAX_RESULTS: usize = 1_000;
const GREP_MAX_MATCHES: usize = 200;

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

    pub fn read_only(&self, name: &str) -> Option<bool> {
        self.tools.get(name).map(|spec| spec.read_only)
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
            "todo_write" => execute_todo_write(&call.tool_call.arguments),
            "complete_step" => execute_complete_step(&call.tool_call.arguments),
            _ => Err(ToolRegistryError::UnimplementedTool(name.to_owned())),
        }
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
            "Read a text file with optional line offset/limit.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "File path"},
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
            "Write content to a file, overwriting existing content.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"]
            }),
        ),
        spec(
            "edit_file",
            false,
            true,
            "Replace an exact string in a file with another.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
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
            "Apply a list of edits to a single file atomically.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
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
            false,
            "Delete a contiguous text range from a file using exact start/end text anchors.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
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
            "Find files matching a glob pattern.",
            json!({
                "type": "object",
                "properties": {"pattern": {"type": "string"}},
                "required": ["pattern"]
            }),
        ),
        spec(
            "grep",
            true,
            true,
            "Search for a regular expression in files.",
            json!({
                "type": "object",
                "properties": {
                    "pattern": {"type": "string"},
                    "path": {"type": "string"}
                },
                "required": ["pattern"]
            }),
        ),
        spec(
            "ls",
            true,
            true,
            "List directory entries.",
            json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "recursive": {"type": "boolean"}
                }
            }),
        ),
        spec(
            "web_fetch",
            true,
            false,
            "Fetch a URL and return readable text content.",
            json!({
                "type": "object",
                "properties": {"url": {"type": "string"}},
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
    })
}

fn execute_read_file(arguments: &[ToolArgument]) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let path = required_string(arguments, "read_file", "path")?;
    let root = locked_workspace_root("read_file")?;
    let path = resolve_locked_path(&root, path, "read_file", "path")?;
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
    Ok(ToolExecutionOutput { text: rendered })
}

fn execute_write_file(
    arguments: &[ToolArgument],
) -> Result<ToolExecutionOutput, ToolRegistryError> {
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
    let existed = path.exists();
    write_text_atomic(&path, content, "write_file")?;
    Ok(ToolExecutionOutput {
        text: format!(
            "{} `{}` ({} bytes)",
            if existed { "Overwrote" } else { "Created" },
            relative_display(&root, &path),
            content.len()
        ),
    })
}

fn execute_edit_file(arguments: &[ToolArgument]) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let path = required_string(arguments, "edit_file", "path")?;
    let old_string = required_non_empty_string(arguments, "edit_file", "old_string")?;
    let new_string = required_present_string(arguments, "edit_file", "new_string")?;
    let root = locked_workspace_root("edit_file")?;
    let path = resolve_locked_path(&root, path, "edit_file", "path")?;
    let original = read_text_file(&path, "edit_file")?;
    let updated = replace_exactly_once(&original, old_string, new_string, "edit_file")?;
    write_text_atomic(&path, &updated, "edit_file")?;
    Ok(ToolExecutionOutput {
        text: format!(
            "Edited `{}` by replacing 1 exact occurrence.",
            relative_display(&root, &path)
        ),
    })
}

fn execute_multi_edit(
    arguments: &[ToolArgument],
) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let path = required_string(arguments, "multi_edit", "path")?;
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
    let root = locked_workspace_root("multi_edit")?;
    let path = resolve_locked_path(&root, path, "multi_edit", "path")?;
    let mut content = read_text_file(&path, "multi_edit")?;
    let mut total_replacements = 0usize;
    for (index, edit) in edits.iter().enumerate() {
        let object = edit
            .as_object()
            .ok_or_else(|| invalid_tool_argument("multi_edit", index, "edit must be an object"))?;
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
        let replacements = if replace_all {
            replace_all_occurrences(&mut content, old_string, new_string, "multi_edit", index)?
        } else {
            content = replace_exactly_once(&content, old_string, new_string, "multi_edit")?;
            1
        };
        total_replacements += replacements;
    }
    write_text_atomic(&path, &content, "multi_edit")?;
    Ok(ToolExecutionOutput {
        text: format!(
            "Edited `{}` with {} edit(s) and {} replacement(s).",
            relative_display(&root, &path),
            edits.len(),
            total_replacements
        ),
    })
}

fn execute_glob(arguments: &[ToolArgument]) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let pattern = required_string(arguments, "glob", "pattern")?;
    validate_glob_pattern(pattern)?;
    let root = locked_workspace_root("glob")?;
    let joined_pattern = root.join(pattern);
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
    Ok(ToolExecutionOutput { text })
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
    let target = resolve_locked_path(&root, target, "grep", "path")?;
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
    Ok(ToolExecutionOutput { text })
}

fn execute_ls(arguments: &[ToolArgument]) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let raw_path = argument_value(arguments, "path")
        .and_then(Value::as_str)
        .unwrap_or(".")
        .trim();
    let recursive = optional_bool(arguments, "ls", "recursive")?.unwrap_or(false);
    let root = locked_workspace_root("ls")?;
    let path = resolve_locked_path(&root, raw_path, "ls", "path")?;
    let metadata = fs::metadata(&path).map_err(|err| ToolRegistryError::ExecutionFailed {
        tool: "ls".to_owned(),
        message: format!("cannot stat `{}`: {err}", path.display()),
    })?;
    if !metadata.is_dir() {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "ls".to_owned(),
            message: format!("`{}` is not a directory", relative_display(&root, &path)),
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
            });
        }
        return Ok(ToolExecutionOutput {
            text: rows.join("\n"),
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
        });
    }
    Ok(ToolExecutionOutput {
        text: rows.join("\n"),
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
    let root = env::current_dir().map_err(|err| ToolRegistryError::ExecutionFailed {
        tool: tool.to_owned(),
        message: format!("cannot read current working directory: {err}"),
    })?;
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
    let candidate = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        root.join(raw)
    };
    let canonical =
        fs::canonicalize(&candidate).map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: tool.to_owned(),
            message: format!("cannot resolve `{field}` `{raw}`: {err}"),
        })?;
    if !canonical.starts_with(root) {
        return Err(ToolRegistryError::InvalidArguments {
            tool: tool.to_owned(),
            message: format!("`{field}` escapes the locked workspace root"),
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
    let candidate = if Path::new(raw).is_absolute() {
        PathBuf::from(raw)
    } else {
        root.join(raw)
    };
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
            message: format!("cannot resolve parent directory for `{field}` `{raw}`: {err}"),
        })?;
    if !canonical_parent.starts_with(root) {
        return Err(ToolRegistryError::InvalidArguments {
            tool: tool.to_owned(),
            message: format!("`{field}` escapes the locked workspace root"),
        });
    }
    Ok(canonical_parent.join(file_name))
}

fn validate_glob_pattern(pattern: &str) -> Result<(), ToolRegistryError> {
    let path = Path::new(pattern);
    if path.is_absolute() {
        return Err(ToolRegistryError::InvalidArguments {
            tool: "glob".to_owned(),
            message: "absolute patterns are not supported".to_owned(),
        });
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(ToolRegistryError::InvalidArguments {
            tool: "glob".to_owned(),
            message: "pattern may not contain `..`".to_owned(),
        });
    }
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::{
        AgentId, FeatureId, SessionId, ToolCallContract, ToolCallId, TraceId, TurnId,
    };
    use std::fs;
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
        assert_eq!(registry.read_only("read_file"), Some(true));
        assert_eq!(registry.read_only("glob"), Some(true));
        assert_eq!(registry.read_only("grep"), Some(true));
        assert_eq!(registry.read_only("ls"), Some(true));
        assert_eq!(registry.read_only("todo_write"), Some(true));
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
    fn complete_step_executes() {
        let registry = BuiltinToolRegistry::reasonix_aligned();
        let output = registry
            .execute(&tool_call(
                "complete_step",
                vec![
                    ToolArgument {
                        name: "step".to_owned(),
                        value: json!("wire tool registry"),
                    },
                    ToolArgument {
                        name: "result".to_owned(),
                        value: json!("done"),
                    },
                    ToolArgument {
                        name: "evidence".to_owned(),
                        value: json!([{"kind": "verification", "summary": "tests passed"}]),
                    },
                ],
            ))
            .expect("complete_step executes");
        assert!(output.text.contains("wire tool registry"));
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
            assert_eq!(
                escape,
                Err(ToolRegistryError::InvalidArguments {
                    tool: "write_file".to_owned(),
                    message: "`path` escapes the locked workspace root".to_owned(),
                })
            );

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
    fn read_file_rejects_path_outside_workspace_root() {
        with_temp_workspace(|root| {
            let parent = root.parent().expect("parent");
            fs::write(parent.join("outside.txt"), "secret\n").expect("write outside");
            let registry = BuiltinToolRegistry::reasonix_aligned();
            let result = registry.execute(&tool_call(
                "read_file",
                vec![ToolArgument {
                    name: "path".to_owned(),
                    value: json!("../outside.txt"),
                }],
            ));
            assert_eq!(
                result,
                Err(ToolRegistryError::InvalidArguments {
                    tool: "read_file".to_owned(),
                    message: "`path` escapes the locked workspace root".to_owned(),
                })
            );
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
        });
    }

    #[test]
    fn unknown_and_unimplemented_tools_fail_explicitly() {
        let registry = BuiltinToolRegistry::reasonix_aligned();
        assert_eq!(
            registry.execute(&tool_call("missing_tool", vec![])),
            Err(ToolRegistryError::UnknownTool("missing_tool".to_owned()))
        );
        assert_eq!(
            registry.execute(&tool_call("web_fetch", vec![])),
            Err(ToolRegistryError::UnimplementedTool("web_fetch".to_owned()))
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
