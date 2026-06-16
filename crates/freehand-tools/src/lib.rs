//! Tool registry and built-in tool surface for Freehand.

use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::path::{Component, Path, PathBuf};

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
            "read_file" => execute_read_file(&call.tool_call.arguments),
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
            false,
            "Run a shell command.",
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
            false,
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
            false,
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
            false,
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
            registry.execute(&tool_call("write_file", vec![])),
            Err(ToolRegistryError::UnimplementedTool(
                "write_file".to_owned()
            ))
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
        let lock = cwd_lock().lock().expect("cwd lock");
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
