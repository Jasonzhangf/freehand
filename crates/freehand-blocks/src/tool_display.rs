use freehand_contracts::{ToolArgument, ToolResultContract, ToolResultStatus};
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolDisplayKind {
    ReadFile,
    FileMutation,
    Search,
    List,
    Plan,
    Shell,
    Generic,
}

impl ToolDisplayKind {
    pub fn as_str(self) -> &'static str {
        match self {
            ToolDisplayKind::ReadFile => "read_file",
            ToolDisplayKind::FileMutation => "file_mutation",
            ToolDisplayKind::Search => "search",
            ToolDisplayKind::List => "list",
            ToolDisplayKind::Plan => "plan",
            ToolDisplayKind::Shell => "shell",
            ToolDisplayKind::Generic => "generic",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolDisplayOutcome {
    Waiting,
    Success,
    Failed,
}

impl ToolDisplayOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            ToolDisplayOutcome::Waiting => "waiting",
            ToolDisplayOutcome::Success => "success",
            ToolDisplayOutcome::Failed => "failed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDisplayField {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDisplayDiff {
    pub target: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDisplayProjection {
    pub kind: ToolDisplayKind,
    pub outcome: ToolDisplayOutcome,
    pub action: String,
    pub target: Option<String>,
    pub parameter_summary: Option<String>,
    pub summary: String,
    pub result_summary: Option<String>,
    pub fields: Vec<ToolDisplayField>,
    pub diff: Option<ToolDisplayDiff>,
}

pub fn project_tool_call_display(
    tool_name: &str,
    arguments: &[ToolArgument],
) -> ToolDisplayProjection {
    if tool_name == "bash" {
        return parse_shell_tool_display(arguments);
    }
    match classify_tool_display_kind(tool_name, arguments) {
        ToolDisplayKind::ReadFile | ToolDisplayKind::List => {
            parse_read_file_tool_display(tool_name, arguments)
        }
        ToolDisplayKind::FileMutation => parse_file_mutation_tool_display(tool_name, arguments),
        ToolDisplayKind::Search => parse_search_tool_display(tool_name, arguments),
        ToolDisplayKind::Plan => parse_plan_tool_display(tool_name, arguments),
        ToolDisplayKind::Shell => parse_shell_tool_display(arguments),
        ToolDisplayKind::Generic => parse_generic_tool_display(tool_name, arguments),
    }
}

pub fn project_tool_result_display(
    mut display: ToolDisplayProjection,
    result: &ToolResultContract,
) -> ToolDisplayProjection {
    display.outcome = match result.status {
        ToolResultStatus::Success => ToolDisplayOutcome::Success,
        ToolResultStatus::Failed => ToolDisplayOutcome::Failed,
    };
    display.result_summary = Some(result_summary_for(&display, result));
    display
}

pub fn classify_tool_display_kind(tool_name: &str, arguments: &[ToolArgument]) -> ToolDisplayKind {
    match tool_name {
        "read_file" => ToolDisplayKind::ReadFile,
        "ls" => ToolDisplayKind::List,
        "write_file" | "edit_file" | "multi_edit" | "delete_range" => ToolDisplayKind::FileMutation,
        "glob" | "grep" => ToolDisplayKind::Search,
        "todo_write" | "complete_step" => ToolDisplayKind::Plan,
        "bash" => classify_shell_command(arguments),
        _ => ToolDisplayKind::Generic,
    }
}

pub fn parse_read_file_tool_display(
    tool_name: &str,
    arguments: &[ToolArgument],
) -> ToolDisplayProjection {
    let kind = if tool_name == "ls" {
        ToolDisplayKind::List
    } else {
        ToolDisplayKind::ReadFile
    };
    let target = string_argument(arguments, "path").unwrap_or_else(|| {
        if tool_name == "ls" {
            ".".to_owned()
        } else {
            "unknown file".to_owned()
        }
    });
    let action = if tool_name == "ls" {
        "List directory".to_owned()
    } else {
        "Read file".to_owned()
    };
    ToolDisplayProjection {
        kind,
        outcome: ToolDisplayOutcome::Waiting,
        action: action.clone(),
        target: Some(target.clone()),
        parameter_summary: parameter_summary_for(vec![
            ("path", Some(target.clone())),
            ("offset", string_argument(arguments, "offset")),
            ("limit", string_argument(arguments, "limit")),
            ("recursive", string_argument(arguments, "recursive")),
        ]),
        summary: format!("{action}: {target}"),
        result_summary: None,
        fields: compact_fields([
            field("tool", tool_name),
            field("target", &target),
            optional_field("offset", string_argument(arguments, "offset")),
            optional_field("limit", string_argument(arguments, "limit")),
            optional_field("recursive", string_argument(arguments, "recursive")),
        ]),
        diff: None,
    }
}

pub fn parse_file_mutation_tool_display(
    tool_name: &str,
    arguments: &[ToolArgument],
) -> ToolDisplayProjection {
    let target = string_argument(arguments, "path").unwrap_or_else(|| "unknown file".to_owned());
    let action = match tool_name {
        "write_file" => "Write file",
        "edit_file" => "Edit file",
        "multi_edit" => "Edit file",
        "delete_range" => "Delete file range",
        _ => "Mutate file",
    }
    .to_owned();
    let diff = match tool_name {
        "edit_file" => match (
            string_argument(arguments, "old_string"),
            string_argument(arguments, "new_string"),
        ) {
            (Some(before), Some(after)) => Some(ToolDisplayDiff {
                target: target.clone(),
                before,
                after,
            }),
            _ => None,
        },
        "write_file" => string_argument(arguments, "content").map(|content| ToolDisplayDiff {
            target: target.clone(),
            before: "<previous file content>".to_owned(),
            after: content,
        }),
        _ => None,
    };
    ToolDisplayProjection {
        kind: ToolDisplayKind::FileMutation,
        outcome: ToolDisplayOutcome::Waiting,
        action: action.clone(),
        target: Some(target.clone()),
        parameter_summary: parameter_summary_for(vec![
            ("path", Some(target.clone())),
            ("old_string", string_argument(arguments, "old_string")),
            ("new_string", string_argument(arguments, "new_string")),
        ]),
        summary: format!("{action}: {target}"),
        result_summary: None,
        fields: compact_fields([
            field("tool", tool_name),
            field("target", &target),
            optional_field(
                "edits",
                array_len_argument(arguments, "edits").map(|len| len.to_string()),
            ),
            optional_field("inclusive", string_argument(arguments, "inclusive")),
        ]),
        diff,
    }
}

pub fn parse_search_tool_display(
    tool_name: &str,
    arguments: &[ToolArgument],
) -> ToolDisplayProjection {
    let target =
        string_argument(arguments, "pattern").unwrap_or_else(|| "unknown pattern".to_owned());
    let path = string_argument(arguments, "path");
    let action = if tool_name == "glob" {
        "Find files".to_owned()
    } else {
        "Search text".to_owned()
    };
    ToolDisplayProjection {
        kind: ToolDisplayKind::Search,
        outcome: ToolDisplayOutcome::Waiting,
        action: action.clone(),
        target: Some(target.clone()),
        parameter_summary: parameter_summary_for(vec![
            ("pattern", Some(target.clone())),
            ("path", string_argument(arguments, "path")),
        ]),
        summary: format!("{action}: {target}"),
        result_summary: None,
        fields: compact_fields([
            field("tool", tool_name),
            field("pattern", &target),
            optional_field("path", path),
        ]),
        diff: None,
    }
}

pub fn parse_plan_tool_display(
    tool_name: &str,
    arguments: &[ToolArgument],
) -> ToolDisplayProjection {
    let (action, target, count) = match tool_name {
        "todo_write" => (
            "Update plan".to_owned(),
            Some("todo list".to_owned()),
            array_len_argument(arguments, "todos"),
        ),
        "complete_step" => (
            "Complete step".to_owned(),
            string_argument(arguments, "step"),
            array_len_argument(arguments, "evidence"),
        ),
        _ => ("Update plan".to_owned(), None, None),
    };
    let target_text = target.clone().unwrap_or_else(|| tool_name.to_owned());
    ToolDisplayProjection {
        kind: ToolDisplayKind::Plan,
        outcome: ToolDisplayOutcome::Waiting,
        action: action.clone(),
        target: Some(target_text.clone()),
        parameter_summary: parameter_summary_for(vec![
            ("target", target.clone()),
            ("items", count.map(|count| count.to_string())),
        ]),
        summary: format!("{action}: {target_text}"),
        result_summary: None,
        fields: compact_fields([
            field("tool", tool_name),
            optional_field("target", target),
            optional_field("items", count.map(|count| count.to_string())),
        ]),
        diff: None,
    }
}

pub fn parse_shell_tool_display(arguments: &[ToolArgument]) -> ToolDisplayProjection {
    let command =
        string_argument(arguments, "command").unwrap_or_else(|| "unknown command".to_owned());
    let kind = classify_shell_command(arguments);
    if command == "pwd" {
        return ToolDisplayProjection {
            kind: ToolDisplayKind::Shell,
            outcome: ToolDisplayOutcome::Waiting,
            action: "Read current working directory".to_owned(),
            target: Some("current workspace".to_owned()),
            parameter_summary: None,
            summary: "Read current working directory: current workspace".to_owned(),
            result_summary: None,
            fields: compact_fields([
                field("tool", "bash"),
                field("target", "current workspace"),
                optional_field("timeout", string_argument(arguments, "timeout_seconds")),
            ]),
            diff: None,
        };
    }
    let action = match kind {
        ToolDisplayKind::ReadFile => "Run file-read command",
        ToolDisplayKind::List => "Run listing command",
        ToolDisplayKind::Search => "Run search command",
        _ => "Run shell command",
    }
    .to_owned();
    let target = shell_command_target(&command, kind);
    ToolDisplayProjection {
        kind,
        outcome: ToolDisplayOutcome::Waiting,
        action: action.clone(),
        target: Some(target.clone()),
        parameter_summary: parameter_summary_for(vec![
            ("target", Some(target.clone())),
            ("timeout", string_argument(arguments, "timeout_seconds")),
        ]),
        summary: format!("{action}: {target}"),
        result_summary: None,
        fields: compact_fields([
            field("tool", "bash"),
            field("target", &target),
            optional_field("timeout", string_argument(arguments, "timeout_seconds")),
        ]),
        diff: None,
    }
}

pub fn parse_generic_tool_display(
    tool_name: &str,
    arguments: &[ToolArgument],
) -> ToolDisplayProjection {
    let fields = arguments
        .iter()
        .take(4)
        .map(|argument| ToolDisplayField {
            label: argument.name.clone(),
            value: compact_value(&argument.value),
        })
        .collect::<Vec<_>>();
    ToolDisplayProjection {
        kind: ToolDisplayKind::Generic,
        outcome: ToolDisplayOutcome::Waiting,
        action: "Run tool".to_owned(),
        target: Some(tool_name.to_owned()),
        parameter_summary: fields_to_parameter_summary(&fields),
        summary: format!("Run tool: {tool_name}"),
        result_summary: None,
        fields,
        diff: None,
    }
}

fn classify_shell_command(arguments: &[ToolArgument]) -> ToolDisplayKind {
    let command = string_argument(arguments, "command").unwrap_or_default();
    let first = command.split_whitespace().next().unwrap_or_default();
    match first {
        "cat" | "head" | "tail" | "sed" => ToolDisplayKind::ReadFile,
        "ls" | "find" => ToolDisplayKind::List,
        "rg" | "grep" => ToolDisplayKind::Search,
        _ => ToolDisplayKind::Shell,
    }
}

fn shell_command_target(command: &str, kind: ToolDisplayKind) -> String {
    let mut parts = command.split_whitespace();
    let _ = parts.next();
    match kind {
        ToolDisplayKind::ReadFile => parts.next().unwrap_or("unknown file").to_owned(),
        ToolDisplayKind::List => parts.next().unwrap_or(".").to_owned(),
        ToolDisplayKind::Search => parts.next().unwrap_or("unknown query").to_owned(),
        ToolDisplayKind::Shell
        | ToolDisplayKind::Generic
        | ToolDisplayKind::Plan
        | ToolDisplayKind::FileMutation => command.trim().to_owned(),
    }
}

fn result_summary_for(display: &ToolDisplayProjection, result: &ToolResultContract) -> String {
    let prefix = match result.status {
        ToolResultStatus::Success => "succeeded",
        ToolResultStatus::Failed => "failed",
    };
    match display.kind {
        ToolDisplayKind::ReadFile => format!("{prefix}: {}", display.target_label("file")),
        ToolDisplayKind::FileMutation => format!("{prefix}: {}", display.target_label("file")),
        ToolDisplayKind::Search => format!("{prefix}: {}", display.target_label("query")),
        ToolDisplayKind::List => format!("{prefix}: {}", display.target_label("path")),
        ToolDisplayKind::Plan => format!("{prefix}: {}", display.target_label("plan")),
        ToolDisplayKind::Shell => format!("{prefix}: shell command"),
        ToolDisplayKind::Generic => {
            if result.output.trim().is_empty() {
                format!("{prefix}: no result text")
            } else {
                format!("{prefix}: result returned")
            }
        }
    }
}

impl ToolDisplayProjection {
    fn target_label(&self, fallback: &str) -> String {
        self.target
            .as_deref()
            .filter(|target| !target.trim().is_empty())
            .unwrap_or(fallback)
            .to_owned()
    }
}

fn string_argument(arguments: &[ToolArgument], name: &str) -> Option<String> {
    let value = arguments
        .iter()
        .find(|argument| argument.name == name)
        .map(|argument| &argument.value)?;
    match value {
        Value::String(value) => Some(value.trim().to_owned()).filter(|value| !value.is_empty()),
        Value::Bool(value) => Some(value.to_string()),
        Value::Number(value) => Some(value.to_string()),
        _ => None,
    }
}

fn array_len_argument(arguments: &[ToolArgument], name: &str) -> Option<usize> {
    arguments
        .iter()
        .find(|argument| argument.name == name)
        .and_then(|argument| argument.value.as_array())
        .map(Vec::len)
}

fn compact_value(value: &Value) -> String {
    match value {
        Value::String(value) => value.clone(),
        Value::Array(items) => format!("{} item(s)", items.len()),
        Value::Object(object) => format!("{} field(s)", object.len()),
        other => other.to_string(),
    }
}

fn parameter_summary_for(items: Vec<(&str, Option<String>)>) -> Option<String> {
    let parts = items
        .into_iter()
        .filter_map(|(label, value)| value.map(|value| format!("{label}={value}")))
        .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

fn fields_to_parameter_summary(fields: &[ToolDisplayField]) -> Option<String> {
    let parts = fields
        .iter()
        .map(|field| format!("{}={}", field.label, field.value))
        .collect::<Vec<_>>();
    if parts.is_empty() {
        None
    } else {
        Some(parts.join(" · "))
    }
}

fn field(label: &str, value: &str) -> Option<ToolDisplayField> {
    Some(ToolDisplayField {
        label: label.to_owned(),
        value: value.to_owned(),
    })
}

fn optional_field(label: &str, value: Option<String>) -> Option<ToolDisplayField> {
    value.map(|value| ToolDisplayField {
        label: label.to_owned(),
        value,
    })
}

fn compact_fields<const N: usize>(fields: [Option<ToolDisplayField>; N]) -> Vec<ToolDisplayField> {
    fields.into_iter().flatten().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use freehand_contracts::{ToolCallId, ToolResultContract};
    use serde_json::json;

    fn arg(name: &str, value: Value) -> ToolArgument {
        ToolArgument {
            name: name.to_owned(),
            value,
        }
    }

    #[test]
    fn read_file_projection_keeps_target_without_file_content() {
        let display = project_tool_call_display(
            "read_file",
            &[arg("path", json!("src/lib.rs")), arg("limit", json!(20))],
        );

        assert_eq!(display.kind, ToolDisplayKind::ReadFile);
        assert_eq!(display.action, "Read file");
        assert_eq!(display.target.as_deref(), Some("src/lib.rs"));
        assert_eq!(
            display.parameter_summary.as_deref(),
            Some("path=src/lib.rs · limit=20")
        );
        assert!(display.diff.is_none());
    }

    #[test]
    fn edit_file_projection_carries_diff_oriented_fields() {
        let display = project_tool_call_display(
            "edit_file",
            &[
                arg("path", json!("src/lib.rs")),
                arg("old_string", json!("old")),
                arg("new_string", json!("new")),
            ],
        );

        assert_eq!(display.kind, ToolDisplayKind::FileMutation);
        let diff = display.diff.expect("diff");
        assert_eq!(diff.target, "src/lib.rs");
        assert_eq!(diff.before, "old");
        assert_eq!(diff.after, "new");
    }

    #[test]
    fn search_projection_extracts_pattern_and_path() {
        let display = project_tool_call_display(
            "grep",
            &[arg("pattern", json!("needle")), arg("path", json!("src"))],
        );

        assert_eq!(display.kind, ToolDisplayKind::Search);
        assert_eq!(display.target.as_deref(), Some("needle"));
        assert_eq!(
            display.parameter_summary.as_deref(),
            Some("pattern=needle · path=src")
        );
        assert!(display.fields.iter().any(|field| field.label == "path"));
    }

    #[test]
    fn plan_projection_summarizes_todo_count() {
        let display = project_tool_call_display(
            "todo_write",
            &[arg(
                "todos",
                json!([
                    {"content": "one", "status": "pending"},
                    {"content": "two", "status": "completed"}
                ]),
            )],
        );

        assert_eq!(display.kind, ToolDisplayKind::Plan);
        assert!(
            display
                .fields
                .iter()
                .any(|field| { field.label == "items" && field.value == "2" })
        );
    }

    #[test]
    fn shell_classifier_recognizes_search_command_shape() {
        let kind = classify_tool_display_kind("bash", &[arg("command", json!("rg TODO src"))]);
        assert_eq!(kind, ToolDisplayKind::Search);
    }

    #[test]
    fn pwd_shell_projection_hides_raw_command_argument() {
        let display = project_tool_call_display("bash", &[arg("command", json!("pwd"))]);

        assert_eq!(display.kind, ToolDisplayKind::Shell);
        assert_eq!(display.action, "Read current working directory");
        assert_eq!(display.target.as_deref(), Some("current workspace"));
        assert!(display.parameter_summary.is_none());
        assert!(!display.summary.contains("command=pwd"));
        assert!(
            !display
                .fields
                .iter()
                .any(|field| field.label == "command" || field.value == "pwd")
        );
    }

    #[test]
    fn result_projection_preserves_category_and_target() {
        let display = project_tool_call_display("read_file", &[arg("path", json!("README.md"))]);
        let result = ToolResultContract {
            tool_call_id: ToolCallId::new("tool-1"),
            status: ToolResultStatus::Failed,
            output: "cannot read".to_owned(),
        };
        let updated = project_tool_result_display(display, &result);

        assert_eq!(updated.kind, ToolDisplayKind::ReadFile);
        assert_eq!(updated.outcome, ToolDisplayOutcome::Failed);
        assert_eq!(updated.target.as_deref(), Some("README.md"));
        assert_eq!(updated.result_summary.as_deref(), Some("failed: README.md"));
    }
}
