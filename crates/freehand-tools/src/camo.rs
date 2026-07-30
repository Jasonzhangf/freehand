//! Camo browser automation tool implementation.
//!
//! Aligns with @web-auto/camo@0.3.5 CLI.
//!
//! # Key CLI conventions (0.3.5)
//! - profileId is ALWAYS positional: `camo goto <profileId> <url>` NOT `--profile <id> --url <url>`
//! - `start` takes `--url` as a flag: `camo start <profileId> --url <url>`
//! - `click/type` take selector as positional: `camo click <profileId> <selector>`
//! - `goto` takes URL as positional: `camo goto <profileId> <url>`
//! - Profile must exist before use (create with `camo profile create <id>`)
//! - No `doctor` or `daemon` commands (daemon auto-starts)
//! - Lifecycle commands (start/stop/goto/click/type) use profile as positional first arg

use serde_json::Value;
use std::process::Command;

use crate::{ToolArgument, ToolExecutionOutput, ToolRegistryError};

// ---------------------------------------------------------------------------
// Command model — each variant corresponds to one camo 0.3.5 subcommand tree
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CamoOp {
    // Lifecycle
    Start,
    Stop,
    Status,
    // Navigation
    Goto,
    Back,
    Screenshot,
    // Interaction
    Click,
    Type,
    Scroll,
    Highlight,
    ClearHighlight,
    Viewport,
    Mouse,
    Window,
    // Pages
    NewPage,
    ClosePage,
    SwitchPage,
    ListPages,
    // DevTools
    Devtools,
    // System
    Sessions,
    Instances,
    Cleanup,
    ForceStop,
    Lock,
    Unlock,
    Shutdown,
    Version,
    Profiles,
    Profile,
    Init,
    Create,
    Config,
    HighlightMode,
    Attach,
    // Cookies
    Cookies,
    // Recording
    Record,
    // Container
    Container,
    // Autoscript
    Autoscript,
    // Events
    Events,
    // System Display
    System,
    // Unknown — catch-all for any subcommand we don't model
    Unknown,
}

impl CamoOp {
    fn from_str(s: &str) -> Self {
        match s {
            // Lifecycle
            "start" => CamoOp::Start,
            "stop" => CamoOp::Stop,
            "status" => CamoOp::Status,
            // Navigation
            "goto" => CamoOp::Goto,
            "back" => CamoOp::Back,
            "screenshot" => CamoOp::Screenshot,
            "qr-screenshot" => CamoOp::Screenshot, // alias
            // Interaction
            "click" => CamoOp::Click,
            "type" => CamoOp::Type,
            "scroll" => CamoOp::Scroll,
            "highlight" => CamoOp::Highlight,
            "clear-highlight" => CamoOp::ClearHighlight,
            "viewport" => CamoOp::Viewport,
            "mouse" => CamoOp::Mouse,
            "window" => CamoOp::Window,
            // Pages
            "new-page" => CamoOp::NewPage,
            "close-page" => CamoOp::ClosePage,
            "switch-page" => CamoOp::SwitchPage,
            "list-pages" => CamoOp::ListPages,
            // DevTools
            "devtools" => CamoOp::Devtools,
            // System
            "sessions" => CamoOp::Sessions,
            "instances" => CamoOp::Instances,
            "cleanup" => CamoOp::Cleanup,
            "force-stop" => CamoOp::ForceStop,
            "lock" => CamoOp::Lock,
            "unlock" => CamoOp::Unlock,
            "shutdown" => CamoOp::Shutdown,
            "version" => CamoOp::Version,
            "profiles" => CamoOp::Profiles,
            "profile" => CamoOp::Profile,
            "init" => CamoOp::Init,
            "create" => CamoOp::Create,
            "config" => CamoOp::Config,
            "highlight-mode" => CamoOp::HighlightMode,
            "attach" => CamoOp::Attach,
            // Cookies
            "cookies" => CamoOp::Cookies,
            // Recording
            "record" => CamoOp::Record,
            // Container
            "container" => CamoOp::Container,
            // Autoscript
            "autoscript" => CamoOp::Autoscript,
            // Events
            "events" => CamoOp::Events,
            // System
            "system" => CamoOp::System,
            // list is alias of status
            "list" => CamoOp::Status,
            _ => CamoOp::Unknown,
        }
    }
}

/// Build the exact argv for `camo <cmd> [profileId] [args...]` per 0.3.5 CLI.
/// Returns None on validation failure with descriptive error.
///
/// # Positional arg rules (0.3.5)
/// - profileId: first positional after command (if present)
/// - url for goto: second positional after profileId
/// - selector for click/type: second positional after profileId  
/// - text for type: third positional after profileId and selector
/// - expression for devtools eval: second positional after profileId
/// - url for start: passed as `--url <url>` flag (NOT positional)
/// - other flags: `--flag value` style
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
        CamoOp::Status => "status",
        CamoOp::Goto => "goto",
        CamoOp::Back => "back",
        CamoOp::Screenshot => "screenshot",
        CamoOp::Click => "click",
        CamoOp::Type => "type",
        CamoOp::Scroll => "scroll",
        CamoOp::Highlight => "highlight",
        CamoOp::ClearHighlight => "clear-highlight",
        CamoOp::Viewport => "viewport",
        CamoOp::Mouse => "mouse",
        CamoOp::Window => "window",
        CamoOp::NewPage => "new-page",
        CamoOp::ClosePage => "close-page",
        CamoOp::SwitchPage => "switch-page",
        CamoOp::ListPages => "list-pages",
        CamoOp::Devtools => "devtools",
        CamoOp::Sessions => "sessions",
        CamoOp::Instances => "instances",
        CamoOp::Cleanup => "cleanup",
        CamoOp::ForceStop => "force-stop",
        CamoOp::Lock => "lock",
        CamoOp::Unlock => "unlock",
        CamoOp::Shutdown => "shutdown",
        CamoOp::Version => "version",
        CamoOp::Profiles => "profiles",
        CamoOp::Profile => "profile",
        CamoOp::Init => "init",
        CamoOp::Create => "create",
        CamoOp::Config => "config",
        CamoOp::HighlightMode => "highlight-mode",
        CamoOp::Attach => "attach",
        CamoOp::Cookies => "cookies",
        CamoOp::Record => "record",
        CamoOp::Container => "container",
        CamoOp::Autoscript => "autoscript",
        CamoOp::Events => "events",
        CamoOp::System => "system",
        CamoOp::Unknown => {
            return Err(ToolRegistryError::InvalidArguments {
                tool: "camo".to_owned(),
                message: "unknown camo command; see `camo help`. \
                 Profile-aware commands (start/stop/goto/click/type/scroll) require a profile."
                    .to_string(),
            });
        }
    };
    argv.push(cmd_word.to_owned());

    let profile = profile
        .filter(|profile| !profile.is_empty())
        .map(str::to_owned);

    // Collect flag args and positional args from the typed map
    let mut positional_url: Option<String> = None;
    let mut positional_selector: Option<String> = None;
    let mut positional_text: Option<String> = None;
    let mut positional_index: Option<i64> = None;
    let mut positional_expression: Option<String> = None;
    let mut devtools_sub: Option<String> = None;
    let mut window_sub: Option<String> = None;
    let mut mouse_sub: Option<String> = None;
    let mut init_sub: Option<String> = None;
    let mut profile_sub: Option<String> = None;
    let mut create_sub: Option<String> = None;
    let mut cookies_sub: Option<String> = None;
    let mut record_sub: Option<String> = None;
    let mut container_sub: Option<String> = None;
    let mut autoscript_sub: Option<String> = None;
    let mut events_sub: Option<String> = None;
    let mut cleanup_sub: Option<String> = None;
    let mut stop_sub: Option<String> = None;
    let mut config_sub: Option<String> = None;
    let mut highlight_mode_sub: Option<String> = None;

    let mut flags: Vec<(String, String)> = Vec::new();

    for (key, val) in args {
        match *key {
            // Skip these — handled above
            "command" | "op" | "profile" => continue,

            // URL: positional for goto; --url flag for start/new-page
            "url" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    let u = s.to_owned();
                    if matches!(op, CamoOp::Start | CamoOp::NewPage) {
                        flags.push(("--url".to_owned(), u));
                    } else {
                        positional_url = Some(u);
                    }
                }
            }
            // Positional selector (click, type, highlight, close-page, switch-page);
            // scroll uses --selector flag (not positional)
            "selector" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    let sel = s.to_owned();
                    if matches!(op, CamoOp::Scroll) {
                        flags.push(("--selector".to_owned(), sel));
                    } else {
                        positional_selector = Some(sel);
                    }
                }
            }
            // Positional text (type)
            "text" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    positional_text = Some(s.to_owned());
                }
            }
            // Positional page index (switch-page, close-page)
            "index" => {
                if let Some(n) = val.as_i64() {
                    positional_index = Some(n);
                }
            }
            // Positional JS expression (devtools eval)
            "expression" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    positional_expression = Some(s.to_owned());
                }
            }
            // DevTools sub-command
            "devtools_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    devtools_sub = Some(s.to_owned());
                }
            }
            "window_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    window_sub = Some(s.to_owned());
                }
            }
            "mouse_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    mouse_sub = Some(s.to_owned());
                }
            }
            "init_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    init_sub = Some(s.to_owned());
                }
            }
            // Devtools logs levels
            "levels" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--levels".to_owned(), s.to_owned()));
                }
            }
            // Devtools logs limit
            "limit" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--limit".to_owned(), n.to_string()));
                }
            }
            // Devtools logs since
            "since" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--since".to_owned(), n.to_string()));
                }
            }

            // Boolean flags
            "clear" => {
                if val.as_bool() == Some(true) {
                    flags.push(("--clear".to_owned(), String::new()));
                }
            }
            "highlight" => {
                if val.as_bool() == Some(false) {
                    flags.push(("--no-highlight".to_owned(), String::new()));
                } else if val.as_bool() == Some(true) {
                    flags.push(("--highlight".to_owned(), String::new()));
                }
            }
            "visible" => {
                if val.as_bool() == Some(true) {
                    flags.push(("--visible".to_owned(), String::new()));
                }
            }
            "no_headless" => {
                if val.as_bool() == Some(true) {
                    flags.push(("--no-headless".to_owned(), String::new()));
                }
            }
            "devtools" => {
                if val.as_bool() == Some(true) {
                    flags.push(("--devtools".to_owned(), String::new()));
                }
            }
            "record" => {
                if val.as_bool() == Some(true) {
                    flags.push(("--record".to_owned(), String::new()));
                }
            }
            "record_overlay" => {
                if val.as_bool() == Some(false) {
                    flags.push(("--no-record-overlay".to_owned(), String::new()));
                } else if val.as_bool() == Some(true) {
                    flags.push(("--record-overlay".to_owned(), String::new()));
                }
            }
            "full" => {
                if val.as_bool() == Some(true) {
                    flags.push(("--full".to_owned(), String::new()));
                }
            }
            "force" => {
                if val.as_bool() == Some(true) {
                    flags.push(("--force".to_owned(), String::new()));
                }
            }

            // String flags
            "alias" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--alias".to_owned(), s.to_owned()));
                }
            }
            "idle_timeout" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--idle-timeout".to_owned(), s.to_owned()));
                }
            }
            "record_name" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    if matches!(op, CamoOp::Record) {
                        flags.push(("--name".to_owned(), s.to_owned()));
                    } else {
                        flags.push(("--record-name".to_owned(), s.to_owned()));
                    }
                }
            }
            "record_output" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--record-output".to_owned(), s.to_owned()));
                }
            }
            "output" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--output".to_owned(), s.to_owned()));
                }
            }
            "padding" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--padding".to_owned(), s.to_owned()));
                }
            }
            "path" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--path".to_owned(), s.to_owned()));
                }
            }
            "source" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--source".to_owned(), s.to_owned()));
                }
            }
            "site" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--site".to_owned(), s.to_owned()));
                }
            }
            "jsonl_file" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--jsonl-file".to_owned(), s.to_owned()));
                }
            }
            "summary_file" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--summary-file".to_owned(), s.to_owned()));
                }
            }
            "snapshot" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    if matches!(op, CamoOp::Autoscript) {
                        if matches!(autoscript_sub.as_deref(), Some("snapshot")) {
                            flags.push(("--out".to_owned(), s.to_owned()));
                        } else {
                            flags.push(("--snapshot".to_owned(), s.to_owned()));
                        }
                    } else {
                        flags.push(("--snapshot".to_owned(), s.to_owned()));
                    }
                }
            }
            "from_node" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--from-node".to_owned(), s.to_owned()));
                }
            }
            "fixture" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--fixture".to_owned(), s.to_owned()));
                }
            }
            "reason" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--reason".to_owned(), s.to_owned()));
                }
            }
            "event" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--event".to_owned(), s.to_owned()));
                }
            }
            "host" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--host".to_owned(), s.to_owned()));
                }
            }
            "port" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--port".to_owned(), n.to_string()));
                }
            }

            // Integer flags
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
            "deltax" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--deltax".to_owned(), n.to_string()));
                }
            }
            "deltay" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--deltay".to_owned(), n.to_string()));
                }
            }
            "amount" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--amount".to_owned(), n.to_string()));
                }
            }
            "clicks" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--clicks".to_owned(), n.to_string()));
                }
            }
            "delay" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--delay".to_owned(), n.to_string()));
                }
            }
            "button" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--button".to_owned(), s.to_owned()));
                }
            }
            "interval" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--interval".to_owned(), n.to_string()));
                }
            }
            "interval_ms" => {
                if let Some(n) = val.as_i64() {
                    flags.push(("--interval".to_owned(), n.to_string()));
                }
            }

            // Scroll direction
            "direction" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    let flag = match s {
                        "down" => "--down",
                        "up" => "--up",
                        "left" => "--left",
                        "right" => "--right",
                        _ => {
                            return Err(ToolRegistryError::InvalidArguments {
                                tool: "camo".to_owned(),
                                message: format!(
                                    "invalid scroll direction `{s}`; use down|up|left|right"
                                ),
                            });
                        }
                    };
                    flags.push((flag.to_owned(), String::new()));
                }
            }

            // Profile sub-command
            "profile_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    profile_sub = Some(s.to_owned());
                }
            }

            // Create sub-command
            "create_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    create_sub = Some(s.to_owned());
                }
            }

            // Cookies sub-command
            "cookies_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    cookies_sub = Some(s.to_owned());
                }
            }

            // Record sub-command
            "record_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    record_sub = Some(s.to_owned());
                }
            }

            // Container sub-command
            "container_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    container_sub = Some(s.to_owned());
                }
            }

            // Autoscript sub-command
            "autoscript_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    autoscript_sub = Some(s.to_owned());
                }
            }

            // Events sub-command
            "events_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    events_sub = Some(s.to_owned());
                }
            }

            // Cleanup sub-command
            "cleanup_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    cleanup_sub = Some(s.to_owned());
                }
            }

            // Stop sub-command
            "stop_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    stop_sub = Some(s.to_owned());
                }
            }

            // Highlight-mode value
            "highlight_mode" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    highlight_mode_sub = Some(s.to_owned());
                }
            }

            "config_op" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    config_sub = Some(s.to_owned());
                }
            }

            // OS and region for create fingerprint
            "os" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--os".to_owned(), s.to_owned()));
                }
            }
            "region" => {
                if let Some(s) = val.as_str().filter(|s| !s.is_empty()) {
                    flags.push(("--region".to_owned(), s.to_owned()));
                }
            }

            _ => {}
        }
    }

    // Subcommands precede the profile. Commands without a nested subcommand
    // take the profile immediately after the top-level command.
    match op {
        CamoOp::Profile => {
            if let Some(sub) = profile_sub {
                argv.push(sub);
            }
            if let Some(profile) = profile {
                argv.push(profile);
            }
        }
        CamoOp::Create => {
            if let Some(sub) = create_sub {
                argv.push(sub);
            }
        }
        CamoOp::Cookies => {
            if let Some(sub) = cookies_sub {
                argv.push(sub);
            }
            if let Some(profile) = profile {
                argv.push(profile);
            }
        }
        CamoOp::Record => {
            if let Some(sub) = record_sub {
                argv.push(sub);
            }
            if let Some(profile) = profile {
                argv.push(profile);
            }
        }
        CamoOp::Container => {
            // Infer container subcommand if not explicitly provided
            let inferred_sub = container_sub.or_else(|| {
                if profile.is_some() {
                    Some("list".to_owned())
                } else {
                    None
                }
            });
            if let Some(sub) = inferred_sub {
                argv.push(sub);
            }
            if let Some(profile) = profile {
                argv.push(profile);
            }
            // container watch accepts --selector flag
            if let Some(s) = positional_selector {
                argv.push("--selector".to_owned());
                argv.push(s);
            }
        }
        CamoOp::Autoscript => {
            if let Some(sub) = autoscript_sub.clone() {
                argv.push(sub);
            }
            // For autoscript run/resume/mock-run: profile is positional after subcommand
            // For other autoscript ops: no profile (validate/explain/snapshot/replay)
            let needs_positional_profile = matches!(
                autoscript_sub.as_deref(),
                Some("run") | Some("resume") | Some("mock-run")
            );
            if needs_positional_profile && let Some(p) = profile {
                argv.push(p);
            }
        }
        CamoOp::Events => {
            if let Some(sub) = events_sub {
                argv.push(sub);
            }
        }
        CamoOp::Window => {
            // Infer window subcommand from flags if not explicitly provided
            let inferred_sub = window_sub.or_else(|| {
                let has_x = args.iter().any(|(k, _)| *k == "x");
                let has_y = args.iter().any(|(k, _)| *k == "y");
                let has_width = args.iter().any(|(k, _)| *k == "width");
                let has_height = args.iter().any(|(k, _)| *k == "height");
                if has_width || has_height {
                    Some("resize".to_owned())
                } else if has_x || has_y {
                    Some("move".to_owned())
                } else {
                    None
                }
            });
            if let Some(sub) = inferred_sub {
                argv.push(sub);
            }
            if let Some(profile) = profile {
                argv.push(profile);
            }
        }
        CamoOp::Mouse => {
            // Infer mouse subcommand from flags if not explicitly provided
            let inferred_sub = mouse_sub.or_else(|| {
                let has_x = args.iter().any(|(k, _)| *k == "x");
                let has_y = args.iter().any(|(k, _)| *k == "y");
                let has_deltax = args.iter().any(|(k, _)| *k == "deltax");
                let has_deltay = args.iter().any(|(k, _)| *k == "deltay");
                if has_deltax || has_deltay {
                    Some("wheel".to_owned())
                } else if has_x || has_y {
                    Some("click".to_owned())
                } else {
                    None
                }
            });
            if let Some(sub) = inferred_sub {
                argv.push(sub);
            }
            if let Some(profile) = profile {
                argv.push(profile);
            }
        }
        CamoOp::Init => {
            if let Some(sub) = init_sub.or(profile_sub) {
                argv.push(sub);
            }
        }
        CamoOp::Config => {
            if let Some(sub) = config_sub.or(profile_sub) {
                argv.push(sub);
            }
        }
        CamoOp::HighlightMode => {
            if let Some(mode) = highlight_mode_sub {
                argv.push(mode);
            }
        }
        CamoOp::Cleanup => {
            if let Some(target) = cleanup_sub.or(profile) {
                argv.push(target);
            }
        }
        CamoOp::Stop => {
            if let Some(target) = stop_sub.or(profile) {
                argv.push(target);
            }
        }
        CamoOp::Devtools => {
            if let Some(sub) = devtools_sub {
                argv.push(sub.clone());
                if let Some(profile) = profile {
                    argv.push(profile);
                }
                if sub == "eval"
                    && let Some(expr) = positional_expression
                {
                    argv.push(expr);
                }
            }
        }
        CamoOp::Goto => {
            if let Some(profile) = profile {
                argv.push(profile);
            }
            if let Some(u) = positional_url {
                argv.push(u);
            }
        }
        CamoOp::Click => {
            if let Some(profile) = profile {
                argv.push(profile);
            }
            if let Some(s) = positional_selector {
                argv.push(s);
            }
        }
        CamoOp::Type => {
            if let Some(profile) = profile {
                argv.push(profile);
            }
            if let Some(s) = positional_selector {
                argv.push(s);
            }
            if let Some(t) = positional_text {
                argv.push(t);
            }
        }
        CamoOp::Highlight => {
            if let Some(profile) = profile {
                argv.push(profile);
            }
            if let Some(s) = positional_selector {
                argv.push(s);
            }
        }
        CamoOp::SwitchPage => {
            if let Some(profile) = profile {
                argv.push(profile);
            }
            if let Some(idx) = positional_index {
                argv.push(idx.to_string());
            }
        }
        CamoOp::ClosePage => {
            if let Some(profile) = profile {
                argv.push(profile);
            }
            if let Some(idx) = positional_index {
                argv.push(idx.to_string());
            }
        }
        CamoOp::System => {
            // system defaults to "display"
            argv.push("display".to_owned());
            if let Some(profile) = profile {
                argv.push(profile);
            }
        }
        _ => {
            if let Some(profile) = profile {
                argv.push(profile);
            }
        }
    }

    // Append all collected flags
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

    // Profile — positional, not a flag
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

    // Helper: build argv for a given op, profile, and args
    fn build_argv(
        op: CamoOp,
        profile: Option<&str>,
        args: &[(&str, Value)],
    ) -> Result<Vec<String>, ToolRegistryError> {
        let args_ref: Vec<(&str, &Value)> = args.iter().map(|(k, v)| (*k, v)).collect();
        build_camo_argv(op, profile, &args_ref)
    }

    // ---- Command construction tests ----

    #[test]
    fn goto_profile_url_positional() {
        // camo goto myprofile https://example.com
        let args = vec![("url", val("https://example.com"))];
        let argv = build_argv(CamoOp::Goto, Some("myprofile"), &args).unwrap();
        assert_eq!(argv, &["camo", "goto", "myprofile", "https://example.com"]);
    }

    #[test]
    fn goto_no_profile_url_positional() {
        // camo goto https://example.com (uses default profile)
        let args = vec![("url", val("https://example.com"))];
        let argv = build_argv(CamoOp::Goto, None, &args).unwrap();
        assert_eq!(argv, &["camo", "goto", "https://example.com"]);
    }

    #[test]
    fn goto_url_as_positional_not_flag() {
        // URL must be positional for goto, NOT --url flag
        let args = vec![("url", val("https://example.com"))];
        let argv = build_argv(CamoOp::Goto, Some("p"), &args).unwrap();
        // Must NOT contain "--url"
        assert!(
            !argv.contains(&"--url".to_owned()),
            "goto URL must be positional, not --url flag: {:?}",
            argv
        );
        assert!(argv.contains(&"https://example.com".to_owned()));
    }

    #[test]
    fn start_profile_url_flag() {
        // camo start myprofile --url https://example.com
        let args = vec![("url", val("https://example.com"))];
        let argv = build_argv(CamoOp::Start, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &["camo", "start", "myprofile", "--url", "https://example.com"]
        );
    }

    #[test]
    fn start_no_profile_url_flag() {
        // camo start --url https://example.com (uses default profile)
        let args = vec![("url", val("https://example.com"))];
        let argv = build_argv(CamoOp::Start, None, &args).unwrap();
        assert_eq!(argv, &["camo", "start", "--url", "https://example.com"]);
    }

    #[test]
    fn click_profile_selector_positional() {
        // camo click myprofile "#search-input"
        let args = vec![("selector", val("#search-input"))];
        let argv = build_argv(CamoOp::Click, Some("myprofile"), &args).unwrap();
        assert_eq!(argv, &["camo", "click", "myprofile", "#search-input"]);
    }

    #[test]
    fn click_selector_as_positional_not_flag() {
        // selector must be positional for click, NOT --selector flag
        let args = vec![("selector", val("#btn"))];
        let argv = build_argv(CamoOp::Click, Some("p"), &args).unwrap();
        assert!(
            !argv.contains(&"--selector".to_owned()),
            "click selector must be positional: {:?}",
            argv
        );
        assert!(argv.contains(&"#btn".to_owned()));
    }

    #[test]
    fn type_profile_selector_text_positional() {
        // camo type myprofile "#input" "hello world"
        let args = vec![("selector", val("#input")), ("text", val("hello world"))];
        let argv = build_argv(CamoOp::Type, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &["camo", "type", "myprofile", "#input", "hello world"]
        );
    }

    #[test]
    fn type_text_as_positional_not_flag() {
        // text must be positional for type, NOT --text flag
        let args = vec![("selector", val("#i")), ("text", val("hi"))];
        let argv = build_argv(CamoOp::Type, Some("p"), &args).unwrap();
        assert!(
            !argv.contains(&"--text".to_owned()),
            "type text must be positional: {:?}",
            argv
        );
        assert!(argv.contains(&"hi".to_owned()));
    }

    #[test]
    fn screenshot_profile_full() {
        // camo screenshot myprofile --full
        let args = vec![("full", vbool(true))];
        let argv = build_argv(CamoOp::Screenshot, Some("myprofile"), &args).unwrap();
        assert_eq!(argv, &["camo", "screenshot", "myprofile", "--full"]);
    }

    #[test]
    fn stop_profile() {
        // camo stop myprofile
        let argv = build_argv(CamoOp::Stop, Some("myprofile"), &[]).unwrap();
        assert_eq!(argv, &["camo", "stop", "myprofile"]);
    }

    #[test]
    fn status_profile() {
        // camo status myprofile
        let argv = build_argv(CamoOp::Status, Some("myprofile"), &[]).unwrap();
        assert_eq!(argv, &["camo", "status", "myprofile"]);
    }

    #[test]
    fn devtools_eval_profile_expression_positional() {
        // camo devtools eval myprofile "document.title"
        let args = vec![
            ("devtools_op", val("eval")),
            ("expression", val("document.title")),
        ];
        let argv = build_argv(CamoOp::Devtools, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &["camo", "devtools", "eval", "myprofile", "document.title"]
        );
    }

    #[test]
    fn devtools_logs_levels_limit() {
        // camo devtools logs myprofile --levels error,warn --limit 50
        let args = vec![
            ("devtools_op", val("logs")),
            ("levels", val("error,warn")),
            ("limit", vi64(50)),
        ];
        let argv = build_argv(CamoOp::Devtools, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "devtools",
                "logs",
                "myprofile",
                "--levels",
                "error,warn",
                "--limit",
                "50"
            ]
        );
    }

    #[test]
    fn scroll_direction_selector_highlight() {
        // camo scroll myprofile --down --selector .feed --highlight
        let args = vec![
            ("direction", val("down")),
            ("selector", val(".feed")),
            ("highlight", vbool(true)),
        ];
        let argv = build_argv(CamoOp::Scroll, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "scroll",
                "myprofile",
                "--down",
                "--selector",
                ".feed",
                "--highlight"
            ]
        );
    }

    #[test]
    fn viewport_width_height() {
        // camo viewport myprofile --width 1920 --height 1080
        let args = vec![("width", vi64(1920)), ("height", vi64(1080))];
        let argv = build_argv(CamoOp::Viewport, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "viewport",
                "myprofile",
                "--width",
                "1920",
                "--height",
                "1080"
            ]
        );
    }

    #[test]
    fn profile_create() {
        // camo profile create myprofile
        let args = vec![("profile_op", val("create"))];
        let argv = build_argv(CamoOp::Profile, Some("myprofile"), &args).unwrap();
        assert_eq!(argv, &["camo", "profile", "create", "myprofile"]);
    }

    #[test]
    fn create_fingerprint_os_region() {
        // camo create fingerprint --os mac --region us
        let args = vec![
            ("create_op", val("fingerprint")),
            ("os", val("mac")),
            ("region", val("us")),
        ];
        let argv = build_argv(CamoOp::Create, None, &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "create",
                "fingerprint",
                "--os",
                "mac",
                "--region",
                "us"
            ]
        );
    }

    #[test]
    fn cookies_get_profile() {
        // camo cookies get myprofile
        let args = vec![("cookies_op", val("get"))];
        let argv = build_argv(CamoOp::Cookies, Some("myprofile"), &args).unwrap();
        assert_eq!(argv, &["camo", "cookies", "get", "myprofile"]);
    }

    #[test]
    fn cookies_save_path() {
        // camo cookies save myprofile --path /tmp/cookies.json
        let args = vec![
            ("cookies_op", val("save")),
            ("path", val("/tmp/cookies.json")),
        ];
        let argv = build_argv(CamoOp::Cookies, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "cookies",
                "save",
                "myprofile",
                "--path",
                "/tmp/cookies.json"
            ]
        );
    }

    #[test]
    fn new_page_url_flag() {
        // camo new-page myprofile --url https://example.com
        let args = vec![("url", val("https://example.com"))];
        let argv = build_argv(CamoOp::NewPage, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "new-page",
                "myprofile",
                "--url",
                "https://example.com"
            ]
        );
    }

    #[test]
    fn start_with_visible_devtools_idle_timeout() {
        // camo start myprofile --visible --devtools --idle-timeout 45m
        let args = vec![
            ("visible", vbool(true)),
            ("devtools", vbool(true)),
            ("idle_timeout", val("45m")),
        ];
        let argv = build_argv(CamoOp::Start, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "start",
                "myprofile",
                "--visible",
                "--devtools",
                "--idle-timeout",
                "45m"
            ]
        );
    }

    #[test]
    fn start_with_record_flags() {
        // camo start myprofile --record --record-name xhs-debug --record-output ./logs/xhs.jsonl --record-overlay
        let args = vec![
            ("record", vbool(true)),
            ("record_name", val("xhs-debug")),
            ("record_output", val("./logs/xhs.jsonl")),
            ("record_overlay", vbool(true)),
        ];
        let argv = build_argv(CamoOp::Start, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "start",
                "myprofile",
                "--record",
                "--record-name",
                "xhs-debug",
                "--record-output",
                "./logs/xhs.jsonl",
                "--record-overlay"
            ]
        );
    }

    #[test]
    fn mouse_click_coords() {
        // camo mouse click myprofile --x 500 --y 300 --button left --clicks 2 --delay 100
        let args = vec![
            ("x", vi64(500)),
            ("y", vi64(300)),
            ("button", val("left")),
            ("clicks", vi64(2)),
            ("delay", vi64(100)),
        ];
        let argv = build_argv(CamoOp::Mouse, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "mouse",
                "click",
                "myprofile",
                "--x",
                "500",
                "--y",
                "300",
                "--button",
                "left",
                "--clicks",
                "2",
                "--delay",
                "100"
            ]
        );
    }

    #[test]
    fn window_resize() {
        // camo window resize myprofile --width 1920 --height 1080
        let args = vec![("width", vi64(1920)), ("height", vi64(1080))];
        let argv = build_argv(CamoOp::Window, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "window",
                "resize",
                "myprofile",
                "--width",
                "1920",
                "--height",
                "1080"
            ]
        );
    }

    #[test]
    fn window_move() {
        // camo window move myprofile --x 100 --y 100
        let args = vec![("x", vi64(100)), ("y", vi64(100))];
        let argv = build_argv(CamoOp::Window, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "window",
                "move",
                "myprofile",
                "--x",
                "100",
                "--y",
                "100"
            ]
        );
    }

    #[test]
    fn record_start_profile() {
        // camo record start myprofile --name session-a --output ./logs/session-a.jsonl
        let args = vec![
            ("record_op", val("start")),
            ("record_name", val("session-a")),
            ("output", val("./logs/session-a.jsonl")),
        ];
        let argv = build_argv(CamoOp::Record, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "record",
                "start",
                "myprofile",
                "--name",
                "session-a",
                "--output",
                "./logs/session-a.jsonl"
            ]
        );
    }

    #[test]
    fn record_stop_reason() {
        // camo record stop myprofile --reason "done"
        let args = vec![("record_op", val("stop")), ("reason", val("done"))];
        let argv = build_argv(CamoOp::Record, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &["camo", "record", "stop", "myprofile", "--reason", "done"]
        );
    }

    #[test]
    fn cleanup_profile() {
        // camo cleanup myprofile
        let argv = build_argv(CamoOp::Cleanup, Some("myprofile"), &[]).unwrap();
        assert_eq!(argv, &["camo", "cleanup", "myprofile"]);
    }

    #[test]
    fn force_stop_profile() {
        // camo force-stop myprofile
        let argv = build_argv(CamoOp::ForceStop, Some("myprofile"), &[]).unwrap();
        assert_eq!(argv, &["camo", "force-stop", "myprofile"]);
    }

    #[test]
    fn highlight_profile_selector() {
        // camo highlight myprofile ".post-card"
        let args = vec![("selector", val(".post-card"))];
        let argv = build_argv(CamoOp::Highlight, Some("myprofile"), &args).unwrap();
        assert_eq!(argv, &["camo", "highlight", "myprofile", ".post-card"]);
    }

    #[test]
    fn clear_highlight_profile() {
        // camo clear-highlight myprofile
        let argv = build_argv(CamoOp::ClearHighlight, Some("myprofile"), &[]).unwrap();
        assert_eq!(argv, &["camo", "clear-highlight", "myprofile"]);
    }

    #[test]
    fn switch_page_index() {
        // camo switch-page myprofile 2
        let args = vec![("index", vi64(2))];
        let argv = build_argv(CamoOp::SwitchPage, Some("myprofile"), &args).unwrap();
        assert_eq!(argv, &["camo", "switch-page", "myprofile", "2"]);
    }

    #[test]
    fn close_page_index() {
        // camo close-page myprofile 0
        let args = vec![("index", vi64(0))];
        let argv = build_argv(CamoOp::ClosePage, Some("myprofile"), &args).unwrap();
        assert_eq!(argv, &["camo", "close-page", "myprofile", "0"]);
    }

    #[test]
    fn list_pages_profile() {
        // camo list-pages myprofile
        let argv = build_argv(CamoOp::ListPages, Some("myprofile"), &[]).unwrap();
        assert_eq!(argv, &["camo", "list-pages", "myprofile"]);
    }

    #[test]
    fn sessions() {
        // camo sessions
        let argv = build_argv(CamoOp::Sessions, None, &[]).unwrap();
        assert_eq!(argv, &["camo", "sessions"]);
    }

    #[test]
    fn instances() {
        // camo instances
        let argv = build_argv(CamoOp::Instances, None, &[]).unwrap();
        assert_eq!(argv, &["camo", "instances"]);
    }

    #[test]
    fn profile_default() {
        // camo profile default
        let argv = build_argv(CamoOp::Profile, None, &[]).unwrap();
        assert_eq!(argv, &["camo", "profile"]);
    }

    #[test]
    fn profile_default_set() {
        // camo profile default myprofile
        let args = vec![("profile_op", val("default"))];
        let argv = build_argv(CamoOp::Profile, Some("myprofile"), &args).unwrap();
        assert_eq!(argv, &["camo", "profile", "default", "myprofile"]);
    }

    #[test]
    fn profile_list() {
        // camo profile list
        let args = vec![("profile_op", val("list"))];
        let argv = build_argv(CamoOp::Profile, None, &args).unwrap();
        assert_eq!(argv, &["camo", "profile", "list"]);
    }

    #[test]
    fn profiles() {
        // camo profiles
        let argv = build_argv(CamoOp::Profiles, None, &[]).unwrap();
        assert_eq!(argv, &["camo", "profiles"]);
    }

    #[test]
    fn init() {
        // camo init
        let argv = build_argv(CamoOp::Init, None, &[]).unwrap();
        assert_eq!(argv, &["camo", "init"]);
    }

    #[test]
    fn init_geoip() {
        // camo init geoip
        let args = vec![("profile_op", val("geoip"))]; // reusing profile_op for subcommand
        let argv = build_argv(CamoOp::Init, None, &args).unwrap();
        assert_eq!(argv, &["camo", "init", "geoip"]);
    }

    #[test]
    fn init_list() {
        // camo init list
        let args = vec![("profile_op", val("list"))];
        let argv = build_argv(CamoOp::Init, None, &args).unwrap();
        assert_eq!(argv, &["camo", "init", "list"]);
    }

    #[test]
    fn config_repo_root() {
        // camo config repo-root /path/to/repo
        let args = vec![
            ("profile_op", val("repo-root")),
            ("path", val("/path/to/repo")),
        ];
        let argv = build_argv(CamoOp::Config, None, &args).unwrap();
        assert_eq!(
            argv,
            &["camo", "config", "repo-root", "--path", "/path/to/repo"]
        );
    }

    #[test]
    fn highlight_mode_on() {
        // camo highlight-mode on
        let args = vec![("highlight_mode", val("on"))];
        let argv = build_argv(CamoOp::HighlightMode, None, &args).unwrap();
        assert_eq!(argv, &["camo", "highlight-mode", "on"]);
    }

    #[test]
    fn highlight_mode_status() {
        // camo highlight-mode status
        let args = vec![("highlight_mode", val("status"))];
        let argv = build_argv(CamoOp::HighlightMode, None, &args).unwrap();
        assert_eq!(argv, &["camo", "highlight-mode", "status"]);
    }

    #[test]
    fn attach_profile() {
        // camo attach myprofile
        let argv = build_argv(CamoOp::Attach, Some("myprofile"), &[]).unwrap();
        assert_eq!(argv, &["camo", "attach", "myprofile"]);
    }

    #[test]
    fn shutdown() {
        // camo shutdown
        let argv = build_argv(CamoOp::Shutdown, None, &[]).unwrap();
        assert_eq!(argv, &["camo", "shutdown"]);
    }

    #[test]
    fn version() {
        // camo version
        let argv = build_argv(CamoOp::Version, None, &[]).unwrap();
        assert_eq!(argv, &["camo", "version"]);
    }

    #[test]
    fn events_serve_host_port() {
        // camo events serve --host 127.0.0.1 --port 7788
        let args = vec![
            ("events_op", val("serve")),
            ("host", val("127.0.0.1")),
            ("port", vi64(7788)),
        ];
        let argv = build_argv(CamoOp::Events, None, &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "events",
                "serve",
                "--host",
                "127.0.0.1",
                "--port",
                "7788"
            ]
        );
    }

    #[test]
    fn events_recent_limit() {
        // camo events recent --limit 50
        let args = vec![("events_op", val("recent")), ("limit", vi64(50))];
        let argv = build_argv(CamoOp::Events, None, &args).unwrap();
        assert_eq!(argv, &["camo", "events", "recent", "--limit", "50"]);
    }

    #[test]
    fn events_tail() {
        // camo events tail error warn
        let args = vec![("events_op", val("tail")), ("levels", val("error warn"))];
        let argv = build_argv(CamoOp::Events, None, &args).unwrap();
        assert_eq!(argv, &["camo", "events", "tail", "--levels", "error warn"]);
    }

    #[test]
    fn autoscript_run_file_profile() {
        // camo autoscript run ./script.as --profile myprofile
        let args = vec![("autoscript_op", val("run")), ("path", val("./script.as"))];
        let argv = build_argv(CamoOp::Autoscript, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "autoscript",
                "run",
                "myprofile",
                "--path",
                "./script.as"
            ]
        );
    }

    #[test]
    fn autoscript_validate_file() {
        // camo autoscript validate ./script.as
        let args = vec![
            ("autoscript_op", val("validate")),
            ("path", val("./script.as")),
        ];
        let argv = build_argv(CamoOp::Autoscript, None, &args).unwrap();
        assert_eq!(
            argv,
            &["camo", "autoscript", "validate", "--path", "./script.as"]
        );
    }

    #[test]
    fn autoscript_explain_file() {
        // camo autoscript explain ./script.as
        let args = vec![
            ("autoscript_op", val("explain")),
            ("path", val("./script.as")),
        ];
        let argv = build_argv(CamoOp::Autoscript, None, &args).unwrap();
        assert_eq!(
            argv,
            &["camo", "autoscript", "explain", "--path", "./script.as"]
        );
    }

    #[test]
    fn autoscript_snapshot_jsonl_out() {
        // camo autoscript snapshot ./run.jsonl --out ./snapshot.json
        let args = vec![
            ("autoscript_op", val("snapshot")),
            ("jsonl_file", val("./run.jsonl")),
            ("snapshot", val("./snapshot.json")),
        ];
        let argv = build_argv(CamoOp::Autoscript, None, &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "autoscript",
                "snapshot",
                "--jsonl-file",
                "./run.jsonl",
                "--out",
                "./snapshot.json"
            ]
        );
    }

    #[test]
    fn autoscript_replay_summary() {
        // camo autoscript replay ./run.jsonl --summary-file ./summary.json
        let args = vec![
            ("autoscript_op", val("replay")),
            ("jsonl_file", val("./run.jsonl")),
            ("summary_file", val("./summary.json")),
        ];
        let argv = build_argv(CamoOp::Autoscript, None, &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "autoscript",
                "replay",
                "--jsonl-file",
                "./run.jsonl",
                "--summary-file",
                "./summary.json"
            ]
        );
    }

    #[test]
    fn autoscript_resume_from_node() {
        // camo autoscript resume ./script.as --snapshot ./snap.json --from-node node-3
        let args = vec![
            ("autoscript_op", val("resume")),
            ("path", val("./script.as")),
            ("snapshot", val("./snap.json")),
            ("from_node", val("node-3")),
        ];
        let argv = build_argv(CamoOp::Autoscript, None, &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "autoscript",
                "resume",
                "--path",
                "./script.as",
                "--snapshot",
                "./snap.json",
                "--from-node",
                "node-3"
            ]
        );
    }

    #[test]
    fn autoscript_mock_run_fixture() {
        // camo autoscript mock-run ./script.as --fixture ./fixture.json --profile myprofile
        let args = vec![
            ("autoscript_op", val("mock-run")),
            ("path", val("./script.as")),
            ("fixture", val("./fixture.json")),
        ];
        let argv = build_argv(CamoOp::Autoscript, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "autoscript",
                "mock-run",
                "myprofile",
                "--path",
                "./script.as",
                "--fixture",
                "./fixture.json"
            ]
        );
    }

    #[test]
    fn container_init_source_force() {
        // camo container init --source ./containers --force
        let args = vec![
            ("container_op", val("init")),
            ("source", val("./containers")),
            ("force", vbool(true)),
        ];
        let argv = build_argv(CamoOp::Container, None, &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "container",
                "init",
                "--source",
                "./containers",
                "--force"
            ]
        );
    }

    #[test]
    fn container_sets_site() {
        // camo container sets --site xhs
        let args = vec![("container_op", val("sets")), ("site", val("xhs"))];
        let argv = build_argv(CamoOp::Container, None, &args).unwrap();
        assert_eq!(argv, &["camo", "container", "sets", "--site", "xhs"]);
    }

    #[test]
    fn container_register_targets() {
        // camo container register myprofile set-1 set-2
        let args = vec![("container_op", val("register"))];
        let argv = build_argv(CamoOp::Container, Some("myprofile"), &args).unwrap();
        assert_eq!(argv, &["camo", "container", "register", "myprofile"]);
    }

    #[test]
    fn container_watch_selector() {
        // camo container watch myprofile --selector .item
        let args = vec![("container_op", val("watch")), ("selector", val(".item"))];
        let argv = build_argv(CamoOp::Container, Some("myprofile"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "container",
                "watch",
                "myprofile",
                "--selector",
                ".item"
            ]
        );
    }

    #[test]
    fn container_list_profile() {
        // camo container list myprofile
        let argv = build_argv(CamoOp::Container, Some("myprofile"), &empty()).unwrap();
        assert_eq!(argv, &["camo", "container", "list", "myprofile"]);
    }

    #[test]
    fn system_display() {
        // camo system display
        let argv = build_argv(CamoOp::System, None, &[]).unwrap();
        assert_eq!(argv, &["camo", "system", "display"]);
    }

    #[test]
    fn lock_list() {
        // camo lock list
        let argv = build_argv(CamoOp::Lock, None, &[]).unwrap();
        assert_eq!(argv, &["camo", "lock"]);
    }

    #[test]
    fn lock_profile() {
        // camo lock myprofile
        let argv = build_argv(CamoOp::Lock, Some("myprofile"), &[]).unwrap();
        assert_eq!(argv, &["camo", "lock", "myprofile"]);
    }

    #[test]
    fn unlock_profile() {
        // camo unlock myprofile
        let argv = build_argv(CamoOp::Unlock, Some("myprofile"), &[]).unwrap();
        assert_eq!(argv, &["camo", "unlock", "myprofile"]);
    }

    // ---- Unknown command ----

    #[test]
    fn unknown_command_rejected() {
        let result = build_argv(CamoOp::Unknown, None, &[]);
        assert!(matches!(
            result,
            Err(ToolRegistryError::InvalidArguments { .. })
        ));
    }

    // ---- Invalid scroll direction ----

    #[test]
    fn invalid_scroll_direction_rejected() {
        let args = vec![("direction", val("invalid"))];
        let result = build_argv(CamoOp::Scroll, Some("p"), &args);
        assert!(matches!(
            result,
            Err(ToolRegistryError::InvalidArguments { .. })
        ));
    }

    // ---- E2E-style argv verification ----
    // These test the full execution path: parse args -> build argv -> would execute

    #[test]
    fn e2e_lifecycle_sequence() {
        // Simulate the full lifecycle: create -> start -> goto -> screenshot -> stop

        // 1. profile create
        let args = vec![("profile_op", val("create"))];
        let argv = build_argv(CamoOp::Profile, Some("e2e-test-p"), &args).unwrap();
        assert_eq!(argv, &["camo", "profile", "create", "e2e-test-p"]);

        // 2. start with URL
        let args = vec![("url", val("https://example.com"))];
        let argv = build_argv(CamoOp::Start, Some("e2e-test-p"), &args).unwrap();
        assert_eq!(
            argv,
            &[
                "camo",
                "start",
                "e2e-test-p",
                "--url",
                "https://example.com"
            ]
        );

        // 3. goto (URL positional)
        let args = vec![("url", val("https://example.com/page"))];
        let argv = build_argv(CamoOp::Goto, Some("e2e-test-p"), &args).unwrap();
        assert_eq!(
            argv,
            &["camo", "goto", "e2e-test-p", "https://example.com/page"]
        );

        // 4. click
        let args = vec![("selector", val("#btn"))];
        let argv = build_argv(CamoOp::Click, Some("e2e-test-p"), &args).unwrap();
        assert_eq!(argv, &["camo", "click", "e2e-test-p", "#btn"]);

        // 5. type
        let args = vec![("selector", val("#inp")), ("text", val("hello"))];
        let argv = build_argv(CamoOp::Type, Some("e2e-test-p"), &args).unwrap();
        assert_eq!(argv, &["camo", "type", "e2e-test-p", "#inp", "hello"]);

        // 6. screenshot
        let args = vec![("full", vbool(true))];
        let argv = build_argv(CamoOp::Screenshot, Some("e2e-test-p"), &args).unwrap();
        assert_eq!(argv, &["camo", "screenshot", "e2e-test-p", "--full"]);

        // 7. stop
        let argv = build_argv(CamoOp::Stop, Some("e2e-test-p"), &[]).unwrap();
        assert_eq!(argv, &["camo", "stop", "e2e-test-p"]);
    }

    // ---- Helper ----
    fn empty() -> Vec<(&'static str, Value)> {
        vec![]
    }
}
