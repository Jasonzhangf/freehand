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

use freehand_contracts::{
    SearchAccessAttempt, SearchAccessStatus, SearchCandidateStatus, SearchDiscoveryCandidate,
    SearchDiscoveryChannel, SearchDiscoveryDelivery, SearchEvidenceDelivery, SearchEvidenceError,
    SearchSocialPlatform, SearchVerificationDelivery,
};
use serde::Deserialize;
use serde_json::Value;
use std::io::Read;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::{Duration, Instant};

use crate::{ToolArgument, ToolExecutionOutput, ToolRegistryError};

const CAMO_DEFAULT_TIMEOUT_SECONDS: u64 = 90;
const CAMO_POLL_INTERVAL_MS: u64 = 100;
const CAMO_SEARCH_ENVELOPE_MARKER: &str = "{\n  \"kind\": \"result\",\n  \"cmd\": \"search\"";
const CAMO_VERIFICATION_READABLE_LIMIT: i64 = 20_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CamoSearchDeliveryKind {
    SocialDiscovery,
    UrlVerification,
}

impl CamoSearchDeliveryKind {
    fn from_arguments(arguments: &[ToolArgument]) -> Result<Option<Self>, ToolRegistryError> {
        let Some(value) = argument_string(arguments, "delivery_kind") else {
            return Ok(None);
        };
        match value {
            "social_discovery" => Ok(Some(Self::SocialDiscovery)),
            "url_verification" => Ok(Some(Self::UrlVerification)),
            other => Err(invalid_camo_arguments(format!(
                "unsupported `delivery_kind` `{other}`; expected `social_discovery` or `url_verification`"
            ))),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CamoSearchEnvelope {
    kind: String,
    cmd: String,
    result: CamoSearchResult,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CamoSearchResult {
    cmd: String,
    searched: bool,
    platform: String,
    query: String,
    success: bool,
    total_count: usize,
    #[serde(rename = "pageURL")]
    page_url: String,
    results: Vec<CamoSearchItem>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CamoSearchItem {
    title: String,
    url: String,
    author: String,
    timestamp: String,
    likes: u64,
    platform: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CamoFetchPageEnvelope {
    cmd: String,
    profile: String,
    url: String,
    ok: bool,
    status: Option<u16>,
    body_length: usize,
    issued_at: String,
    trace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CamoPageInfoEnvelope {
    cmd: String,
    profile: String,
    info: CamoPageInfo,
    issued_at: String,
    trace_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CamoPageInfo {
    cmd: String,
    ok: bool,
    url: String,
    title: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct CamoReadableEnvelope {
    cmd: String,
    profile: String,
    text: String,
    length: usize,
    issued_at: String,
    trace_id: String,
}

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
    if let Some(delivery_kind) = CamoSearchDeliveryKind::from_arguments(arguments)? {
        return match delivery_kind {
            CamoSearchDeliveryKind::SocialDiscovery => execute_camo_social_discovery(arguments),
            CamoSearchDeliveryKind::UrlVerification => execute_camo_url_verification(arguments),
        };
    }

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

    let timeout_arg = arguments
        .iter()
        .find(|a| a.name == "timeout_seconds")
        .and_then(|a| a.value.as_u64())
        .filter(|n| *n > 0)
        .unwrap_or(CAMO_DEFAULT_TIMEOUT_SECONDS);

    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|err| ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: format!("cannot run `{}`: {err}", argv.join(" ")),
        })?;

    let deadline = Instant::now() + Duration::from_secs(timeout_arg);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) => {
                if Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    return Err(ToolRegistryError::ExecutionFailed {
                        tool: "camo".to_owned(),
                        message: format!(
                            "camo `{}` exceeded {}s execution timeout and was killed",
                            argv.join(" "),
                            timeout_arg
                        ),
                    });
                }
                sleep(Duration::from_millis(CAMO_POLL_INTERVAL_MS));
            }
            Err(err) => break Err(err),
        }
    };

    let status = status.map_err(|err| ToolRegistryError::ExecutionFailed {
        tool: "camo".to_owned(),
        message: format!("cannot wait on `{}`: {err}", argv.join(" ")),
    })?;

    let mut stdout_bytes = Vec::new();
    if let Some(mut out) = child.stdout.take() {
        let _ = out.read_to_end(&mut stdout_bytes);
    }
    let mut stderr_bytes = Vec::new();
    if let Some(mut err) = child.stderr.take() {
        let _ = err.read_to_end(&mut stderr_bytes);
    }
    let stdout = String::from_utf8_lossy(&stdout_bytes);
    let stderr = String::from_utf8_lossy(&stderr_bytes);

    if !status.success() {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: format!(
                "camo exited with {}\nstdout: {}\nstderr: {}",
                status, stdout, stderr
            ),
        });
    }

    Ok(ToolExecutionOutput {
        text: stdout.to_string(),
        search_evidence: None,
    })
}

fn execute_camo_social_discovery(
    arguments: &[ToolArgument],
) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let command = required_camo_string(arguments, "command")?;
    if command != "search" {
        return Err(invalid_camo_arguments(
            "`delivery_kind=social_discovery` requires `command=search`",
        ));
    }
    let platform = required_camo_string(arguments, "platform")?;
    if platform != "xhs" {
        return Err(invalid_camo_arguments(format!(
            "camo social platform `{platform}` is unsupported by the installed CLI; only `xhs` is currently declared"
        )));
    }
    let query = required_camo_string(arguments, "query")?;
    let domain_plan_ref = required_camo_string(arguments, "domain_plan_ref")?;
    let delivery_id = required_camo_string(arguments, "delivery_id")?;
    let profile = argument_string(arguments, "profile").unwrap_or("default");
    let stdout = execute_camo_command(
        CamoOp::Search,
        Some(profile),
        arguments,
        CAMO_DEFAULT_TIMEOUT_SECONDS,
    )?;
    let envelope = parse_camo_search_envelope(&stdout)?;
    if envelope.kind != "result"
        || envelope.cmd != "search"
        || envelope.result.cmd != "search"
        || !envelope.result.searched
        || !envelope.result.success
    {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: "camo search did not return a successful typed result envelope".to_owned(),
        });
    }
    if envelope.result.platform != platform || envelope.result.query != query {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: "camo search result does not match the requested platform/query".to_owned(),
        });
    }
    if envelope.result.total_count != envelope.result.results.len() {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: "camo search result count does not match its result list".to_owned(),
        });
    }
    if envelope.result.results.is_empty() {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: "camo search returned no social candidates".to_owned(),
        });
    }
    let candidates = envelope
        .result
        .results
        .into_iter()
        .enumerate()
        .map(|(index, item)| {
            if item.platform != "xhs" {
                return Err(ToolRegistryError::ExecutionFailed {
                    tool: "camo".to_owned(),
                    message: format!(
                        "camo search result {} reported unexpected platform `{}`",
                        index + 1,
                        item.platform
                    ),
                });
            }
            let usable = is_http_url(&item.url);
            let snippet = [item.author, item.timestamp, format!("likes={}", item.likes)]
                .into_iter()
                .filter(|value| !value.trim().is_empty())
                .collect::<Vec<_>>()
                .join(" | ");
            Ok(SearchDiscoveryCandidate {
                candidate_id: format!("{delivery_id}-candidate-{}", index + 1),
                status: if usable {
                    SearchCandidateStatus::Usable
                } else {
                    SearchCandidateStatus::UnusableOther
                },
                original_url: usable.then_some(item.url),
                title: item.title,
                snippet,
                discovered_by: Some(SearchDiscoveryChannel::CamoSocialSearch),
                platform: Some(SearchSocialPlatform::Xhs),
                source_weight: None,
                reason: (!usable).then_some("camo_search_returned_non_http_url".to_owned()),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let delivery = SearchDiscoveryDelivery {
        schema: "search_evidence.discovery.v1".to_owned(),
        delivery_id: delivery_id.to_owned(),
        discovery_channel: SearchDiscoveryChannel::CamoSocialSearch,
        domain_plan_ref: Some(domain_plan_ref.to_owned()),
        hosted_search_attempt: None,
        candidates,
    };

    Ok(ToolExecutionOutput {
        text: format!(
            "camo social discovery completed platform=xhs candidates={} page_url={}",
            delivery.candidates.len(),
            envelope.result.page_url
        ),
        search_evidence: Some(SearchEvidenceDelivery::Discovery(delivery)),
    })
}

fn execute_camo_url_verification(
    arguments: &[ToolArgument],
) -> Result<ToolExecutionOutput, ToolRegistryError> {
    let command = required_camo_string(arguments, "command")?;
    if command != "fetch-page" {
        return Err(invalid_camo_arguments(
            "`delivery_kind=url_verification` requires `command=fetch-page`",
        ));
    }
    let source_id = required_camo_string(arguments, "source_id")?;
    let delivery_id = required_camo_string(arguments, "delivery_id")?;
    let original_url = required_camo_string(arguments, "url")?;
    if !is_http_url(original_url) {
        return Err(invalid_camo_arguments("`url` must be HTTP or HTTPS"));
    }
    let profile = argument_string(arguments, "profile").unwrap_or("default");
    let fetch_stdout = execute_camo_command(
        CamoOp::FetchPage,
        Some(profile),
        arguments,
        CAMO_DEFAULT_TIMEOUT_SECONDS,
    )?;
    let fetch: CamoFetchPageEnvelope = parse_camo_json(&fetch_stdout, "fetch-page")?;
    validate_fetch_page_identity(&fetch, profile, original_url)?;
    if !fetch.ok {
        let status = fetch
            .status
            .map(|code| format!("http_{code}"))
            .unwrap_or_else(|| "fetch_page_failed".to_owned());
        return Ok(camo_verification_failure_output(
            delivery_id,
            source_id,
            original_url,
            profile,
            fetch.issued_at,
            SearchAccessStatus::HttpError,
            status,
            "camo could not access the requested page".to_owned(),
        ));
    }

    let page_info_stdout = match execute_camo_command(
        CamoOp::GetPageInfo,
        Some(profile),
        &[],
        CAMO_DEFAULT_TIMEOUT_SECONDS,
    ) {
        Ok(stdout) => stdout,
        Err(_) => {
            return Ok(camo_verification_failure_output(
                delivery_id,
                source_id,
                original_url,
                profile,
                fetch.issued_at,
                SearchAccessStatus::Blocked,
                "page_info_failed".to_owned(),
                "camo could not inspect the accessed page".to_owned(),
            ));
        }
    };
    let page_info: CamoPageInfoEnvelope = match parse_camo_json(&page_info_stdout, "get-page-info")
    {
        Ok(envelope) => envelope,
        Err(_) => {
            return Ok(camo_verification_failure_output(
                delivery_id,
                source_id,
                original_url,
                profile,
                fetch.issued_at,
                SearchAccessStatus::Blocked,
                "page_info_invalid".to_owned(),
                "camo returned invalid page information".to_owned(),
            ));
        }
    };
    if validate_page_info_envelope(&page_info, profile).is_err() {
        return Ok(camo_verification_failure_output(
            delivery_id,
            source_id,
            original_url,
            profile,
            fetch.issued_at,
            SearchAccessStatus::Blocked,
            "page_info_unusable".to_owned(),
            "camo page information did not prove a usable page".to_owned(),
        ));
    }

    let readable_arguments = [ToolArgument {
        name: "maxLength".to_owned(),
        value: Value::from(CAMO_VERIFICATION_READABLE_LIMIT),
    }];
    let readable_stdout = match execute_camo_command(
        CamoOp::GetReadable,
        Some(profile),
        &readable_arguments,
        CAMO_DEFAULT_TIMEOUT_SECONDS,
    ) {
        Ok(stdout) => stdout,
        Err(_) => {
            return Ok(camo_verification_failure_output(
                delivery_id,
                source_id,
                original_url,
                profile,
                fetch.issued_at,
                SearchAccessStatus::Blocked,
                "readable_failed".to_owned(),
                "camo could not extract readable page evidence".to_owned(),
            ));
        }
    };
    let readable: CamoReadableEnvelope = match parse_camo_json(&readable_stdout, "get-readable") {
        Ok(envelope) => envelope,
        Err(_) => {
            return Ok(camo_verification_failure_output(
                delivery_id,
                source_id,
                original_url,
                profile,
                fetch.issued_at,
                SearchAccessStatus::Blocked,
                "readable_invalid".to_owned(),
                "camo returned an invalid readable evidence envelope".to_owned(),
            ));
        }
    };
    if validate_readable_envelope(&readable, profile).is_err() {
        return Ok(camo_verification_failure_output(
            delivery_id,
            source_id,
            original_url,
            profile,
            readable.issued_at,
            SearchAccessStatus::Blocked,
            "evidence_empty".to_owned(),
            "camo page contained no readable evidence".to_owned(),
        ));
    }

    let accessed_at = readable.issued_at.clone();
    let delivery = SearchVerificationDelivery {
        schema: "search_evidence.verification.v1".to_owned(),
        delivery_id: delivery_id.to_owned(),
        source_id: source_id.to_owned(),
        original_url: original_url.to_owned(),
        camo_profile: profile.to_owned(),
        accessed_at: accessed_at.clone(),
        access_status: SearchAccessStatus::Verified,
        page_title: Some(page_info.info.title),
        evidence_excerpt: Some(readable.text),
        verified_by: Some("camo".to_owned()),
        access_attempts: vec![SearchAccessAttempt {
            attempt_id: fetch.trace_id,
            channel: "camo".to_owned(),
            status: SearchAccessStatus::Verified,
            accessed_at,
            error: None,
        }],
        error: None,
    };

    Ok(ToolExecutionOutput {
        text: format!("camo verified source_id={source_id} url={original_url}"),
        search_evidence: Some(SearchEvidenceDelivery::Verification(delivery)),
    })
}

#[allow(clippy::too_many_arguments)]
fn camo_verification_failure_output(
    delivery_id: &str,
    source_id: &str,
    original_url: &str,
    profile: &str,
    accessed_at: String,
    status: SearchAccessStatus,
    code: String,
    message: String,
) -> ToolExecutionOutput {
    let error = SearchEvidenceError { code, message };
    let delivery = SearchVerificationDelivery {
        schema: "search_evidence.verification.v1".to_owned(),
        delivery_id: delivery_id.to_owned(),
        source_id: source_id.to_owned(),
        original_url: original_url.to_owned(),
        camo_profile: profile.to_owned(),
        accessed_at: accessed_at.clone(),
        access_status: status,
        page_title: None,
        evidence_excerpt: None,
        verified_by: None,
        access_attempts: vec![SearchAccessAttempt {
            attempt_id: format!("{delivery_id}-attempt-1"),
            channel: "camo".to_owned(),
            status,
            accessed_at,
            error: Some(error.clone()),
        }],
        error: Some(error),
    };
    ToolExecutionOutput {
        text: format!("camo could not verify source_id={source_id} url={original_url}"),
        search_evidence: Some(SearchEvidenceDelivery::Verification(delivery)),
    }
}

fn execute_camo_command(
    op: CamoOp,
    profile: Option<&str>,
    arguments: &[ToolArgument],
    default_timeout_seconds: u64,
) -> Result<String, ToolRegistryError> {
    let args = arguments
        .iter()
        .map(|argument| (argument.name.as_str(), &argument.value))
        .collect::<Vec<_>>();
    let argv = build_camo_argv(op, profile, &args)?;
    let timeout_seconds = arguments
        .iter()
        .find(|argument| argument.name == "timeout_seconds")
        .and_then(|argument| argument.value.as_u64())
        .filter(|seconds| *seconds > 0)
        .unwrap_or(default_timeout_seconds);
    let output = run_camo_process(&argv, timeout_seconds)?;
    String::from_utf8(output).map_err(|error| ToolRegistryError::ExecutionFailed {
        tool: "camo".to_owned(),
        message: format!("camo stdout is not valid UTF-8: {error}"),
    })
}

fn run_camo_process(argv: &[String], timeout_seconds: u64) -> Result<Vec<u8>, ToolRegistryError> {
    let mut command = Command::new(&argv[0]);
    command
        .args(&argv[1..])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .map_err(|error| ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: format!("cannot run `{}`: {error}", argv.join(" ")),
        })?;
    let deadline = Instant::now() + Duration::from_secs(timeout_seconds);
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if Instant::now() < deadline => {
                sleep(Duration::from_millis(CAMO_POLL_INTERVAL_MS));
            }
            Ok(None) => {
                child
                    .kill()
                    .map_err(|error| ToolRegistryError::ExecutionFailed {
                        tool: "camo".to_owned(),
                        message: format!("cannot stop timed-out camo process: {error}"),
                    })?;
                child
                    .wait()
                    .map_err(|error| ToolRegistryError::ExecutionFailed {
                        tool: "camo".to_owned(),
                        message: format!("cannot reap timed-out camo process: {error}"),
                    })?;
                return Err(ToolRegistryError::ExecutionFailed {
                    tool: "camo".to_owned(),
                    message: format!(
                        "camo `{}` exceeded {}s execution timeout",
                        argv.join(" "),
                        timeout_seconds
                    ),
                });
            }
            Err(error) => {
                return Err(ToolRegistryError::ExecutionFailed {
                    tool: "camo".to_owned(),
                    message: format!("cannot wait on `{}`: {error}", argv.join(" ")),
                });
            }
        }
    };
    let mut stdout = Vec::new();
    if let Some(mut pipe) = child.stdout.take() {
        pipe.read_to_end(&mut stdout)
            .map_err(|error| ToolRegistryError::ExecutionFailed {
                tool: "camo".to_owned(),
                message: format!("cannot read camo stdout: {error}"),
            })?;
    }
    let mut stderr = Vec::new();
    if let Some(mut pipe) = child.stderr.take() {
        pipe.read_to_end(&mut stderr)
            .map_err(|error| ToolRegistryError::ExecutionFailed {
                tool: "camo".to_owned(),
                message: format!("cannot read camo stderr: {error}"),
            })?;
    }
    if !status.success() {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: format!(
                "camo exited with {status}; stdout: {}; stderr: {}",
                String::from_utf8_lossy(&stdout),
                String::from_utf8_lossy(&stderr)
            ),
        });
    }
    Ok(stdout)
}

fn parse_camo_search_envelope(stdout: &str) -> Result<CamoSearchEnvelope, ToolRegistryError> {
    let start = stdout.find(CAMO_SEARCH_ENVELOPE_MARKER).ok_or_else(|| {
        ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: "camo search stdout is missing its typed JSON result envelope".to_owned(),
        }
    })?;
    parse_camo_json(&stdout[start..], "search")
}

fn parse_camo_json<T: for<'de> Deserialize<'de>>(
    stdout: &str,
    command: &str,
) -> Result<T, ToolRegistryError> {
    serde_json::from_str(stdout.trim()).map_err(|error| ToolRegistryError::ExecutionFailed {
        tool: "camo".to_owned(),
        message: format!("camo `{command}` returned an invalid typed JSON envelope: {error}"),
    })
}

fn validate_fetch_page_identity(
    envelope: &CamoFetchPageEnvelope,
    profile: &str,
    original_url: &str,
) -> Result<(), ToolRegistryError> {
    if envelope.cmd != "fetch-page" || envelope.profile != profile || envelope.url != original_url {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: "camo fetch-page result does not match the requested profile and URL"
                .to_owned(),
        });
    }
    let _ = envelope.body_length;
    Ok(())
}

fn validate_page_info_envelope(
    envelope: &CamoPageInfoEnvelope,
    profile: &str,
) -> Result<(), ToolRegistryError> {
    if envelope.cmd != "get-page-info"
        || envelope.profile != profile
        || envelope.info.cmd != "get-page-info"
        || !envelope.info.ok
        || envelope.info.title.trim().is_empty()
        || !is_http_url(&envelope.info.url)
    {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: "camo get-page-info result does not match the accessed URL or has no title"
                .to_owned(),
        });
    }
    let _ = (&envelope.issued_at, &envelope.trace_id);
    Ok(())
}

fn validate_readable_envelope(
    envelope: &CamoReadableEnvelope,
    profile: &str,
) -> Result<(), ToolRegistryError> {
    if envelope.cmd != "get-readable"
        || envelope.profile != profile
        || envelope.text.trim().is_empty()
        || envelope.length == 0
    {
        return Err(ToolRegistryError::ExecutionFailed {
            tool: "camo".to_owned(),
            message: "camo get-readable result contains no usable page evidence".to_owned(),
        });
    }
    let _ = &envelope.trace_id;
    Ok(())
}

fn is_http_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}

fn argument_string<'a>(arguments: &'a [ToolArgument], name: &str) -> Option<&'a str> {
    arguments
        .iter()
        .find(|argument| argument.name == name)
        .and_then(|argument| argument.value.as_str())
        .filter(|value| !value.trim().is_empty())
}

fn required_camo_string<'a>(
    arguments: &'a [ToolArgument],
    name: &str,
) -> Result<&'a str, ToolRegistryError> {
    argument_string(arguments, name).ok_or_else(|| {
        invalid_camo_arguments(format!("camo typed delivery requires non-empty `{name}`"))
    })
}

fn invalid_camo_arguments(message: impl Into<String>) -> ToolRegistryError {
    ToolRegistryError::InvalidArguments {
        tool: "camo".to_owned(),
        message: message.into(),
    }
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

    fn tool_arguments(values: &[(&str, Value)]) -> Vec<ToolArgument> {
        values
            .iter()
            .map(|(name, value)| ToolArgument {
                name: (*name).to_owned(),
                value: value.clone(),
            })
            .collect()
    }

    #[test]
    fn parses_successful_xhs_search_into_typed_social_discovery() {
        let stdout = r#"[BrowserInstance] Login status: LOGGED_IN
{
  "kind": "result",
  "cmd": "search",
  "result": {
    "cmd": "search",
    "searched": true,
    "platform": "xhs",
    "query": "OpenAI",
    "success": true,
    "totalCount": 1,
    "pageURL": "https://www.xiaohongshu.com/search_result?keyword=OpenAI",
    "results": [{
      "title": "OpenAI update",
      "url": "https://www.xiaohongshu.com/search_result/abc",
      "author": "author",
      "timestamp": "today",
      "likes": 3,
      "platform": "xhs"
    }]
  }
}"#;
        let envelope = parse_camo_search_envelope(stdout).expect("typed search envelope");
        assert!(envelope.result.success);
        assert_eq!(envelope.result.results[0].platform, "xhs");
    }

    #[test]
    fn social_discovery_rejects_unsupported_platform_before_execution() {
        let arguments = tool_arguments(&[
            ("delivery_kind", val("social_discovery")),
            ("delivery_id", val("social-1")),
            ("domain_plan_ref", val("domain-1")),
            ("command", val("search")),
            ("platform", val("weibo")),
            ("query", val("news")),
        ]);

        assert!(matches!(
            execute_camo_impl(&arguments),
            Err(ToolRegistryError::InvalidArguments { message, .. })
                if message.contains("only `xhs`")
        ));
    }

    #[test]
    fn typed_delivery_rejects_wrong_command_without_running_camo() {
        let arguments = tool_arguments(&[
            ("delivery_kind", val("url_verification")),
            ("delivery_id", val("verify-1")),
            ("source_id", val("source-1")),
            ("command", val("get-readable")),
            ("url", val("https://example.com")),
        ]);

        assert!(matches!(
            execute_camo_impl(&arguments),
            Err(ToolRegistryError::InvalidArguments { message, .. })
                if message.contains("requires `command=fetch-page`")
        ));
    }

    #[test]
    fn verified_delivery_requires_fetch_page_info_and_readable_envelopes() {
        let fetch: CamoFetchPageEnvelope = parse_camo_json(
            r#"{
              "cmd":"fetch-page","profile":"default","url":"https://example.com",
              "ok":true,"status":null,"bodyLength":0,
              "issuedAt":"2026-08-15T10:25:27Z","traceId":"fetch-1"
            }"#,
            "fetch-page",
        )
        .expect("fetch envelope");
        let page: CamoPageInfoEnvelope = parse_camo_json(
            r#"{
              "cmd":"get-page-info","profile":"default",
              "info":{"cmd":"get-page-info","ok":true,"url":"https://example.com/","title":"Example Domain"},
              "issuedAt":"2026-08-15T10:25:35Z","traceId":"page-1"
            }"#,
            "get-page-info",
        )
        .expect("page envelope");
        let readable: CamoReadableEnvelope = parse_camo_json(
            r#"{
              "cmd":"get-readable","profile":"default","text":"verified evidence","length":17,
              "issuedAt":"2026-08-15T10:25:42Z","traceId":"readable-1"
            }"#,
            "get-readable",
        )
        .expect("readable envelope");

        validate_fetch_page_identity(&fetch, "default", "https://example.com")
            .expect("fetch identity");
        validate_page_info_envelope(&page, "default").expect("page info");
        validate_readable_envelope(&readable, "default").expect("readable evidence");
    }

    #[test]
    fn empty_readable_envelope_cannot_be_verified() {
        let readable: CamoReadableEnvelope = parse_camo_json(
            r#"{
              "cmd":"get-readable","profile":"default","text":"","length":0,
              "issuedAt":"2026-08-15T10:25:42Z","traceId":"readable-1"
            }"#,
            "get-readable",
        )
        .expect("readable envelope");

        assert!(validate_readable_envelope(&readable, "default").is_err());
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

#[cfg(test)]
mod live_tests {
    use super::*;
    use crate::ToolArgument;
    use serde_json::json;

    fn args(pairs: &[(&str, &str)]) -> Vec<ToolArgument> {
        pairs
            .iter()
            .map(|(k, v)| ToolArgument {
                name: k.to_string(),
                value: json!(v),
            })
            .collect()
    }

    fn live(command: &str, pairs: &[(&str, &str)]) -> Result<String, String> {
        let mut all = vec![("command", command)];
        all.extend_from_slice(pairs);
        let out = execute_camo_impl(&args(&all)).map_err(|e| format!("{e:?}"))?;
        Ok(out.text)
    }

    #[test]
    #[ignore = "live camo 0.4.2 integration; requires camo CLI + profile"]
    fn live_camo_full_session() {
        let profile = "freehand-camo-integration";
        let _ = live("stop", &[("profile", profile)]);
        let start = live("start", &[("profile", profile)]).unwrap();
        assert!(start.contains("sessionId"), "start: {start}");
        let goto = live(
            "goto",
            &[("profile", profile), ("url", "https://example.com")],
        )
        .unwrap();
        assert!(goto.contains("\"navigated\": true"), "goto: {goto}");
        let readable = live(
            "get-readable",
            &[("profile", profile), ("maxLength", "500")],
        )
        .unwrap();
        assert!(readable.contains("Example Domain"), "readable: {readable}");
        let ev = live(
            "evaluate",
            &[("profile", profile), ("script", "document.title")],
        )
        .unwrap();
        assert!(ev.contains("Example Domain"), "evaluate: {ev}");
        let shot = live(
            "screenshot",
            &[
                ("profile", profile),
                ("path", "/tmp/camo-freehand-live.png"),
            ],
        )
        .unwrap();
        assert!(shot.contains("\"saved\": true"), "screenshot: {shot}");
        let stop = live("stop", &[("profile", profile)]).unwrap();
        assert!(stop.contains("stopped"), "stop: {stop}");
    }
}
