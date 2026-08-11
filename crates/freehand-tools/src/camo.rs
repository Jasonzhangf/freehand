//! Camo browser automation tool implementation.
//!
//! Aligns with @web-auto/camo@0.4.2 CLI (camo.v2 protocol/v1).
//!
//! # Key CLI conventions (0.4.2)
//! - `profile` is a NAMED flag: `camo goto <url> --profile <id>` NOT positional.
//! - Most arguments are named flags (`--selector`, `--text`, `--script`, `--tabId`, ...).
//! - Only a few take a positional argument:
//!   - `goto <url>`, `type <text>`, `daemon <start|stop|status>`,
//!     `search <platform> <query>`, `fetch-page <url>`.
//! - Profile must exist before use; `camo start` requires an active session.
//! - `doctor` runs environment sanity checks; no `status`/`sessions`/`instances`
//!   system commands in 0.4.2 (daemon status via `camo daemon status`).

use serde_json::Value;
use std::process::Command;

use crate::{ToolArgument, ToolExecutionOutput, ToolRegistryError};

// ---------------------------------------------------------------------------
// Command model — each variant corresponds to one camo 0.4.2 subcommand tree
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CamoOp {
    // Lifecycle
    Start,
    Stop,
    // Navigation
    Goto,
    Screenshot,
    // Interaction
    Click,
    Type,
    Scroll,
    Hover,
    // Page inspection
    Snapshot,
    GetText,
    GetPageInfo,
    FindElements,
    GetReadable,
    GetCookies,
    SetCookies,
    SetUserAgent,
    SetViewport,
    WaitDomStable,
    // Tabs
    NewTab,
    CloseTab,
    ListTabs,
    SwitchTab,
    // DevTools / eval
    Evaluate,
    // Wait
    Wait,
    // Upload / select
    Upload,
    Select,
    // Daemon
    Daemon,
    // Search
    Search,
    ScrollAndCollect,
    FetchPage,
    // Unknown — catch-all for any subcommand we don't model
    Unknown,
}

impl CamoOp {
    fn from_str(s: &str) -> Self {
        match s {
            // Lifecycle
            "start" => CamoOp::Start,
            "stop" => CamoOp::Stop,
            // Navigation
            "goto" => CamoOp::Goto,
            "screenshot" => CamoOp::Screenshot,
            // Interaction
            "click" => CamoOp::Click,
            "type" => CamoOp::Type,
            "scroll" => CamoOp::Scroll,
            "hover" => CamoOp::Hover,
            // Page inspection
            "snapshot" => CamoOp::Snapshot,
            "get-text" => CamoOp::GetText,
            "get-page-info" => CamoOp::GetPageInfo,
            "find-elements" => CamoOp::FindElements,
            "get-readable" => CamoOp::GetReadable,
            "get-cookies" => CamoOp::GetCookies,
            "set-cookies" => CamoOp::SetCookies,
            "set-user-agent" => CamoOp::SetUserAgent,
            "set-viewport" => CamoOp::SetViewport,
            "wait-dom-stable" => CamoOp::WaitDomStable,
            // Tabs
            "new-tab" => CamoOp::NewTab,
            "close-tab" => CamoOp::CloseTab,
            "list-tabs" => CamoOp::ListTabs,
            "switch-tab" => CamoOp::SwitchTab,
            // DevTools / eval
            "evaluate" => CamoOp::Evaluate,
            // Wait
            "wait" => CamoOp::Wait,
            // Upload / select
            "upload" => CamoOp::Upload,
            "select" => CamoOp::Select,
            // Daemon
            "daemon" => CamoOp::Daemon,
            // Search
            "search" => CamoOp::Search,
            "scroll-and-collect" => CamoOp::ScrollAndCollect,
            "fetch-page" => CamoOp::FetchPage,
            _ => CamoOp::Unknown,
        }
    }
}

/// Build the exact argv for `camo <cmd> [flags...]` per 0.4.2 CLI.
/// Returns None on validation failure with descriptive error.
///
/// # Named-arg rules (0.4.2)
/// - `profile` is always a `--profile <id>` flag (never positional).
/// - goto/fetch-page take `url` as positional; type takes `text` as positional;
///   search takes `platform`+`query` positional; daemon takes `subcommand` positional.
/// - All other fields are named flags.
pub fn build_camo_argv(
    op: CamoOp,
    profile: Option<&str>,
    args: &[(&str, &Value)],
) -> Result<Vec<String>, ToolRegistryError> {
    let mut argv = vec!["camo".to_owned()];

    // Top-level command word
    let cmd_word = match op {
        CamoOp::Start => "start",
        CamoOp::Stop => "stop",
        CamoOp::Goto => "goto",
        CamoOp::Screenshot => "screenshot",
        CamoOp::Click => "click",
        CamoOp::Type => "type",
        CamoOp::Scroll => "scroll",
        CamoOp::Hover => "hover",
        CamoOp::Snapshot => "snapshot",
        CamoOp::GetText => "get-text",
        CamoOp::GetPageInfo => "get-page-info",
        CamoOp::FindElements => "find-elements",
        CamoOp::GetReadable => "get-readable",
        CamoOp::GetCookies => "get-cookies",
        CamoOp::SetCookies => "set-cookies",
        CamoOp::SetUserAgent => "set-user-agent",
        CamoOp::SetViewport => "set-viewport",
        CamoOp::WaitDomStable => "wait-dom-stable",
        CamoOp::NewTab => "new-tab",
        CamoOp::CloseTab => "close-tab",
        CamoOp::ListTabs => "list-tabs",
        CamoOp::SwitchTab => "switch-tab",
        CamoOp::Evaluate => "evaluate",
        CamoOp::Wait => "wait",
        CamoOp::Upload => "upload",
        CamoOp::Select => "select",
        CamoOp::Daemon => "daemon",
        CamoOp::Search => "search",
        CamoOp::ScrollAndCollect => "scroll-and-collect",
        CamoOp::FetchPage => "fetch-page",
        CamoOp::Unknown => {
            return Err(ToolRegistryError::InvalidArguments {
                tool: "camo".to_owned(),
                message: "unknown camo command; see `camo help`. \
                 Profile-aware commands (start/stop/goto/click/type/scroll/...) require a profile."
                    .to_string(),
            });
        }
    };
    argv.push(cmd_word.to_owned());

    let profile = profile
        .filter(|profile| !profile.is_empty())
        .map(str::to_owned);

    // Collect positional args and named flags per 0.4.2 schema.
    let mut positional_url: Option<String> = None;
    let mut positional_text: Option<String> = None;
    let mut daemon_subcommand: Option<String> = None;
    let mut search_platform: Option<String> = None;
    let mut search_query: Option<String> = None;

    let mut flags: Vec<(String, String)> = Vec::new();

    // profile flag goes first for all profile-aware commands.
    if let Some(p) = profile {
        flags.push(("--profile".to_owned(), p));
    }

    for (key, val) in args {
        match *key {
            // Skip these — handled above
            "command" | "op" | "profile" => continue,

            // Positional url (goto, fetch-page)
            "url" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    if matches!(op, CamoOp::Start | CamoOp::NewTab) {
                        flags.push(("--url".to_owned(), s.to_owned()));
                    } else {
                        positional_url = Some(s.to_owned());
                    }
                }
            }
            // Positional text (type)
            "text" => {
                if matches!(op, CamoOp::Type) {
                    if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                        positional_text = Some(s.to_owned());
                    }
                } else {
                    if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                        flags.push(("--text".to_owned(), s.to_owned()));
                    }
                }
            }
            // click/hover/find-elements selector (flag)
            "selector" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--selector".to_owned(), s.to_owned()));
                }
            }
            // daemon subcommand (positional)
            "subcommand" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    daemon_subcommand = Some(s.to_owned());
                }
            }
            // search positional platform + query
            "platform" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    search_platform = Some(s.to_owned());
                }
            }
            "query" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    search_query = Some(s.to_owned());
                }
            }
            // wait condition
            "wait_until" | "waitUntil" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--waitUntil".to_owned(), s.to_owned()));
                }
            }
            "wait_for" | "for" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--for".to_owned(), s.to_owned()));
                }
            }
            "target" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--target".to_owned(), s.to_owned()));
                }
            }
            // script (evaluate)
            "script" | "expression" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--script".to_owned(), s.to_owned()));
                }
            }
            // file (upload)
            "file" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--file".to_owned(), s.to_owned()));
                }
            }
            // select value
            "value" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--value".to_owned(), s.to_owned()));
                }
            }
            // cookies (search/set-cookies)
            "cookies" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--cookies".to_owned(), s.to_owned()));
                }
            }
            // user agent
            "ua" | "user_agent" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--ua".to_owned(), s.to_owned()));
                }
            }
            // path (screenshot output)
            "path" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--path".to_owned(), s.to_owned()));
                }
            }
            // format (snapshot)
            "format" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--format".to_owned(), s.to_owned()));
                }
            }
            // button (click)
            "button" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--button".to_owned(), s.to_owned()));
                }
            }
            // scroll deltas
            "x" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--x".to_owned(), n.to_string()));
                }
            }
            "y" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--y".to_owned(), n.to_string()));
                }
            }
            // integer flags
            "timeout" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--timeout".to_owned(), n.to_string()));
                }
            }
            "delay" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--delay".to_owned(), n.to_string()));
                }
            }
            "tab_id" | "tabId" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--tabId".to_owned(), n.to_string()));
                }
            }
            "width" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--width".to_owned(), n.to_string()));
                }
            }
            "height" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--height".to_owned(), n.to_string()));
                }
            }
            "max_length" | "maxLength" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--maxLength".to_owned(), n.to_string()));
                }
            }
            "poll" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--poll".to_owned(), n.to_string()));
                }
            }
            "scroll_count" | "scrollCount" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--scrollCount".to_owned(), n.to_string()));
                }
            }
            "max_results" | "max-results" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--max-results".to_owned(), n.to_string()));
                }
            }
            "limit" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--limit".to_owned(), n.to_string()));
                }
            }
            // boolean flags
            "headless" => {
                if val.as_bool() == Some(true) {
                    flags.push(("--headless".to_owned(), String::new()));
                }
            }
            "ephemeral" => {
                if val.as_bool() == Some(true) {
                    flags.push(("--ephemeral".to_owned(), String::new()));
                }
            }

            _ => {}
        }
    }

    // Push positionals in the exact order required by 0.4.2.
    match op {
        CamoOp::Goto | CamoOp::FetchPage => {
            if let Some(u) = positional_url {
                argv.push(u);
            }
        }
        CamoOp::Type => {
            if let Some(t) = positional_text {
                argv.push(t);
            }
        }
        CamoOp::Search => {
            if let Some(p) = search_platform {
                argv.push(p);
            }
            if let Some(q) = search_query {
                argv.push(q);
            }
        }
        CamoOp::Daemon => {
            if let Some(sub) = daemon_subcommand {
                argv.push(sub);
            }
        }
        _ => {}
    }

    // Append all named flags.
    for (flag, value) in flags {
        argv.push(flag);
        if !value.is_empty() {
            argv.push(value);
        }
    }

    Ok(argv)
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

pub fn execute_camo_impl(
    arguments: &[ToolArgument],
) -> Result<ToolExecutionOutput, ToolRegistryError> {
    // Parse command
    let command = arguments
        .iter()
        .find(|a| a.name == "command")
        .and_then(|a| a.value.as_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| ToolRegistryError::InvalidArguments {
            tool: "camo".to_owned(),
            message: "camo tool requires `command` field (e.g. start, goto, click, type, screenshot, stop)".to_owned(),
        })?;

    let op = CamoOp::from_str(command);
    if op == CamoOp::Unknown {
        return Err(ToolRegistryError::InvalidArguments {
            tool: "camo".to_owned(),
            message: format!(
                "unknown camo command `{command}`; see `camo help` for valid commands. \
                 Profile-aware commands require profile created with `camo profile create <id>`."
            ),
        });
    }

    // Profile — named flag in 0.4.2.
    let profile = arguments
        .iter()
        .find(|a| a.name == "profile")
        .and_then(|a| a.value.as_str())
        .filter(|s| !s.is_empty());

    // Build typed (key, value) pairs for the command builder
    let args: Vec<(&str, &Value)> = arguments
        .iter()
        .map(|a| (a.name.as_str(), &a.value))
        .collect();

    let argv = build_camo_argv(op, profile, &args)?;

    let output = Command::new(&argv[0])
        .args(&argv[1..])
        .output()
        .map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: format!("cannot run `{}`: {err}", argv.join(" ")),
        })?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    if !output.status.success() {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: format!(
                "camo exited with {}\nstdout: {}\nstderr: {}",
                output.status, stdout, stderr
            ),
        });
    }

    Ok(ToolExecutionOutput {
        text: stdout.to_string(),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn val(s: &str) -> Value {
        json!(s)
    }
    fn vi64(n: i64) -> Value {
        json!(n)
    }
    fn vbool(b: bool) -> Value {
        json!(b)
    }

    fn build_argv(
        op: CamoOp,
        profile: Option<&str>,
        args: &[(&str, Value)],
    ) -> Result<Vec<String>, ToolRegistryError> {
        let args_ref: Vec<(&str, &Value)> = args.iter().map(|(k, v)| (*k, v)).collect();
        build_camo_argv(op, profile, &args_ref)
    }

    // ---- Core 0.4.2 named-profile / positional-url rules ----

    #[test]
    fn start_profile_url_headless_all_flags() {
        // camo start --profile myprofile --url https://... --headless
        let args = vec![
            ("url", val("https://example.com")),
            ("headless", vbool(true)),
        ];
        let argv = build_argv(CamoOp::Start, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "start",
                "--profile",
                "myprofile",
                "--url",
                "https://example.com",
                "--headless",
            ]
        );
    }

    #[test]
    fn stop_profile_only_flag() {
        // camo stop --profile myprofile
        let argv = build_argv(CamoOp::Stop, Some("myprofile"), &[]).unwrap();
        assert_eq!(argv, &["camo", "stop", "--profile", "myprofile"]);
    }

    #[test]
    fn goto_url_positional_with_profile_flag_and_wait_until() {
        // camo goto https://example.com --profile myprofile --waitUntil networkidle
        let args = vec![
            ("url", val("https://example.com")),
            ("waitUntil", val("networkidle")),
        ];
        let argv = build_argv(CamoOp::Goto, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "goto",
                "https://example.com",
                "--profile",
                "myprofile",
                "--waitUntil",
                "networkidle",
            ]
        );
    }

    #[test]
    fn click_selector_and_button_are_flags() {
        // camo click --profile myprofile --selector #btn --button left
        let args = vec![("selector", val("#btn")), ("button", val("left"))];
        let argv = build_argv(CamoOp::Click, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "click",
                "--profile",
                "myprofile",
                "--selector",
                "#btn",
                "--button",
                "left",
            ]
        );
    }

    #[test]
    fn type_text_positional_then_selector_delay() {
        // camo type hello --profile myprofile --selector #inp --delay 30
        let args = vec![
            ("text", val("hello")),
            ("selector", val("#inp")),
            ("delay", vi64(30)),
        ];
        let argv = build_argv(CamoOp::Type, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "type",
                "hello",
                "--profile",
                "myprofile",
                "--selector",
                "#inp",
                "--delay",
                "30",
            ]
        );
    }

    #[test]
    fn scroll_xy_flags() {
        let args = vec![("x", vi64(0)), ("y", vi64(420))];
        let argv = build_argv(CamoOp::Scroll, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "scroll",
                "--profile",
                "myprofile",
                "--x",
                "0",
                "--y",
                "420"
            ]
        );
    }

    #[test]
    fn screenshot_path_flag() {
        let args = vec![("path", val("/tmp/shot.png"))];
        let argv = build_argv(CamoOp::Screenshot, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "screenshot",
                "--profile",
                "myprofile",
                "--path",
                "/tmp/shot.png"
            ]
        );
    }

    #[test]
    fn snapshot_format_flag() {
        let args = vec![("format", val("yaml"))];
        let argv = build_argv(CamoOp::Snapshot, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "snapshot",
                "--profile",
                "myprofile",
                "--format",
                "yaml"
            ]
        );
    }

    #[test]
    fn evaluate_script_flag() {
        let args = vec![("script", val("document.title"))];
        let argv = build_argv(CamoOp::Evaluate, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "evaluate",
                "--profile",
                "myprofile",
                "--script",
                "document.title",
            ]
        );
    }

    #[test]
    fn wait_for_timeout_target() {
        let args = vec![
            ("for", val("selector")),
            ("timeout", vi64(5000)),
            ("target", val("#ready")),
        ];
        let argv = build_argv(CamoOp::Wait, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "wait",
                "--profile",
                "myprofile",
                "--for",
                "selector",
                "--timeout",
                "5000",
                "--target",
                "#ready",
            ]
        );
    }

    #[test]
    fn upload_selector_file() {
        let args = vec![
            ("selector", val("input[type=file]")),
            ("file", val("/tmp/x.png")),
        ];
        let argv = build_argv(CamoOp::Upload, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "upload",
                "--profile",
                "myprofile",
                "--selector",
                "input[type=file]",
                "--file",
                "/tmp/x.png",
            ]
        );
    }

    #[test]
    fn select_selector_value() {
        let args = vec![("selector", val("#sel")), ("value", val("us"))];
        let argv = build_argv(CamoOp::Select, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "select",
                "--profile",
                "myprofile",
                "--selector",
                "#sel",
                "--value",
                "us",
            ]
        );
    }

    #[test]
    fn daemon_subcommand_positional() {
        let args = vec![("subcommand", val("status"))];
        let argv = build_argv(CamoOp::Daemon, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &["camo", "daemon", "status", "--profile", "myprofile"]
        );
    }

    #[test]
    fn search_platform_query_positional_then_flags() {
        let args = vec![
            ("platform", val("xhs")),
            ("query", val("rust camo")),
            ("max-results", vi64(5)),
        ];
        let argv = build_argv(CamoOp::Search, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "search",
                "xhs",
                "rust camo",
                "--profile",
                "myprofile",
                "--max-results",
                "5",
            ]
        );
    }

    #[test]
    fn fetch_page_url_positional_with_timeout() {
        let args = vec![
            ("url", val("https://example.com")),
            ("timeout", vi64(15000)),
        ];
        let argv = build_argv(CamoOp::FetchPage, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "fetch-page",
                "https://example.com",
                "--profile",
                "myprofile",
                "--timeout",
                "15000",
            ]
        );
    }

    #[test]
    fn get_readable_max_length() {
        let args = vec![("maxLength", vi64(8000))];
        let argv = build_argv(CamoOp::GetReadable, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "get-readable",
                "--profile",
                "myprofile",
                "--maxLength",
                "8000"
            ]
        );
    }

    #[test]
    fn get_text_selector() {
        let args = vec![("selector", val("h1"))];
        let argv = build_argv(CamoOp::GetText, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "get-text",
                "--profile",
                "myprofile",
                "--selector",
                "h1"
            ]
        );
    }

    #[test]
    fn set_viewport_width_height() {
        let args = vec![("width", vi64(1280)), ("height", vi64(720))];
        let argv = build_argv(CamoOp::SetViewport, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "set-viewport",
                "--profile",
                "myprofile",
                "--width",
                "1280",
                "--height",
                "720",
            ]
        );
    }

    #[test]
    fn set_user_agent_ua() {
        let args = vec![("ua", val("Mozilla/5.0 ..."))];
        let argv = build_argv(CamoOp::SetUserAgent, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "set-user-agent",
                "--profile",
                "myprofile",
                "--ua",
                "Mozilla/5.0 ...",
            ]
        );
    }

    #[test]
    fn wait_dom_stable_timeout_poll() {
        let args = vec![("timeout", vi64(2000)), ("poll", vi64(200))];
        let argv = build_argv(CamoOp::WaitDomStable, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "wait-dom-stable",
                "--profile",
                "myprofile",
                "--timeout",
                "2000",
                "--poll",
                "200",
            ]
        );
    }

    #[test]
    fn new_tab_url_flag() {
        let args = vec![("url", val("https://example.com"))];
        let argv = build_argv(CamoOp::NewTab, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "new-tab",
                "--profile",
                "myprofile",
                "--url",
                "https://example.com",
            ]
        );
    }

    #[test]
    fn close_tab_tab_id() {
        let args = vec![("tabId", vi64(0))];
        let argv = build_argv(CamoOp::CloseTab, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "close-tab",
                "--profile",
                "myprofile",
                "--tabId",
                "0"
            ]
        );
    }

    #[test]
    fn switch_tab_tab_id() {
        let args = vec![("tabId", vi64(2))];
        let argv = build_argv(CamoOp::SwitchTab, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "switch-tab",
                "--profile",
                "myprofile",
                "--tabId",
                "2"
            ]
        );
    }

    #[test]
    fn list_tabs_only_profile() {
        let argv = build_argv(CamoOp::ListTabs, Some("myprofile"), &[]).unwrap();
        assert_eq!(argv, &["camo", "list-tabs", "--profile", "myprofile"]);
    }

    #[test]
    fn get_page_info_only_profile() {
        let argv = build_argv(CamoOp::GetPageInfo, Some("myprofile"), &[]).unwrap();
        assert_eq!(argv, &["camo", "get-page-info", "--profile", "myprofile"]);
    }

    #[test]
    fn get_cookies_only_profile() {
        let argv = build_argv(CamoOp::GetCookies, Some("myprofile"), &[]).unwrap();
        assert_eq!(argv, &["camo", "get-cookies", "--profile", "myprofile"]);
    }

    #[test]
    fn set_cookies_flag() {
        let args = vec![("cookies", val("[{\"name\":\"x\"}]"))];
        let argv = build_argv(CamoOp::SetCookies, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "set-cookies",
                "--profile",
                "myprofile",
                "--cookies",
                "[{\"name\":\"x\"}]",
            ]
        );
    }

    #[test]
    fn hover_selector_flag() {
        let args = vec![("selector", val(".cta"))];
        let argv = build_argv(CamoOp::Hover, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "hover",
                "--profile",
                "myprofile",
                "--selector",
                ".cta"
            ]
        );
    }

    #[test]
    fn find_elements_selector_flag() {
        let args = vec![("selector", val("article"))];
        let argv = build_argv(CamoOp::FindElements, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "find-elements",
                "--profile",
                "myprofile",
                "--selector",
                "article",
            ]
        );
    }

    #[test]
    fn scroll_and_collect_count_delay() {
        let args = vec![("scrollCount", vi64(3)), ("delay", vi64(500))];
        let argv = build_argv(CamoOp::ScrollAndCollect, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "scroll-and-collect",
                "--profile",
                "myprofile",
                "--scrollCount",
                "3",
                "--delay",
                "500",
            ]
        );
    }

    #[test]
    fn unknown_command_returns_error() {
        let argv = build_argv(CamoOp::Unknown, Some("myprofile"), &[]);
        assert!(argv.is_err());
    }

    #[test]
    fn ephemerel_flag_dropped_when_false() {
        let args = vec![("ephemeral", vbool(false))];
        let argv = build_argv(CamoOp::Daemon, Some("myprofile"), &args).unwrap();
        assert_eq!(argv, &["camo", "daemon", "--profile", "myprofile"]);
    }

    #[test]
    fn profile_omitted_yields_no_profile_flag() {
        let argv = build_argv(CamoOp::Stop, None, &[]).unwrap();
        assert_eq!(argv, &["camo", "stop"]);
    }

    #[test]
    fn empty_profile_treated_as_no_profile() {
        let argv = build_argv(CamoOp::Stop, Some(""), &[]).unwrap();
        assert_eq!(argv, &["camo", "stop"]);
    }
}
