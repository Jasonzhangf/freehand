use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

fn main() {
    let mut args = env::args().skip(1);
    match (args.next().as_deref(), args.next().as_deref()) {
        (Some("gates"), Some("check")) => {
            if let Err(err) = run_gates_check() {
                eprintln!("xtask gates check failed: {err}");
                std::process::exit(1);
            }
            println!("xtask gates check: ok");
        }
        (Some("mainlines"), Some("generate")) => {
            if let Err(err) = run_mainlines_generate() {
                eprintln!("xtask mainlines generate failed: {err}");
                std::process::exit(1);
            }
            println!("xtask mainlines generate: ok");
        }
        (Some("mainlines"), Some("check")) => {
            if let Err(err) = run_mainlines_check() {
                eprintln!("xtask mainlines check failed: {err}");
                std::process::exit(1);
            }
            println!("xtask mainlines check: ok");
        }
        _ => {
            eprintln!(
                "usage: cargo run -p xtask -- <gates check|mainlines generate|mainlines check>"
            );
            std::process::exit(1);
        }
    }
}

fn run_mainlines_generate() -> Result<(), String> {
    let root = env::current_dir().map_err(|err| err.to_string())?;
    generate_mainline_wikis(&root, true)
}

fn run_mainlines_check() -> Result<(), String> {
    let root = env::current_dir().map_err(|err| err.to_string())?;
    generate_mainline_wikis(&root, false)
}

fn run_gates_check() -> Result<(), String> {
    let root = env::current_dir().map_err(|err| err.to_string())?;
    require_files(
        &root,
        &[
            "AGENTS.md",
            "CACHE.md",
            "MEMORY.md",
            "note.md",
            "docs/architecture/feature-map.md",
            "docs/architecture/function-map-spec.md",
            "docs/function-maps/README.md",
            "docs/mainline-calls/README.md",
            "docs/function-maps/foundation.workspace.md",
            "docs/function-maps/config.core.md",
            "docs/function-maps/provider.semantic.md",
            "docs/function-maps/provider.openai-adapter.md",
            "docs/function-maps/provider.anthropic-adapter.md",
            "docs/function-maps/tool.registry.md",
            "docs/function-maps/contracts.core.md",
            "docs/function-maps/debug.core.md",
            "docs/function-maps/reason.turn.md",
            "docs/function-maps/reason.session-history.md",
            "docs/function-maps/reason.persistence.md",
            "docs/function-maps/reason.rewrite-policy.md",
            "docs/function-maps/reason.context-planner.md",
            "docs/function-maps/ui.protocol.md",
            "docs/function-maps/node.master-slave.md",
            "docs/function-maps/runtime.ui-command-dispatch.md",
            "docs/function-maps/app.runtime-daemon.md",
            "docs/mainline-calls/provider.anthropic-adapter.json",
            "docs/mainline-calls/provider.openai-adapter.json",
            "docs/mainline-calls/provider.semantic.json",
            "docs/mainline-calls/provider.reason-live-bridge.json",
            "docs/mainline-calls/tool.registry.json",
            "docs/mainline-calls/ui.protocol.json",
            "docs/mainline-calls/debug.core.json",
            "docs/mainline-calls/reason.turn.json",
            "docs/mainline-calls/reason.session-history.json",
            "docs/mainline-calls/reason.persistence.json",
            "docs/mainline-calls/reason.rewrite-policy.json",
            "docs/mainline-calls/reason.context-planner.json",
            "docs/mainline-calls/runtime.ui-command-dispatch.json",
            "docs/wiki/README.md",
            "docs/wiki/provider.anthropic-adapter.md",
            "docs/wiki/provider.openai-adapter.md",
            "docs/wiki/provider.semantic.md",
            "docs/wiki/provider.reason-live-bridge.md",
            "docs/wiki/tool.registry.md",
            "docs/wiki/ui.protocol.md",
            "docs/wiki/debug.core.md",
            "docs/wiki/reason.turn.md",
            "docs/wiki/reason.session-history.md",
            "docs/wiki/reason.persistence.md",
            "docs/wiki/reason.rewrite-policy.md",
            "docs/wiki/reason.context-planner.md",
            "docs/wiki/runtime.ui-command-dispatch.md",
            "docs/architecture/debug-and-trace.md",
            "docs/architecture/dev-gates.md",
            "docs/architecture/dev-debug-workflow.md",
            "docs/architecture/test-strategy.md",
            "docs/testing/foundation.workspace.md",
            "docs/testing/config.core.md",
            "docs/testing/provider.semantic.md",
            "docs/testing/provider.openai-adapter.md",
            "docs/testing/provider.anthropic-adapter.md",
            "docs/testing/tool.registry.md",
            "docs/testing/contracts.core.md",
            "docs/testing/debug.core.md",
            "docs/testing/reason.turn.md",
            "docs/testing/reason.session-history.md",
            "docs/testing/reason.persistence.md",
            "docs/testing/reason.rewrite-policy.md",
            "docs/testing/reason.context-planner.md",
            "docs/testing/ui.protocol.md",
            "docs/testing/node.master-slave.md",
            "docs/testing/runtime.ui-command-dispatch.md",
            "docs/testing/app.runtime-daemon.md",
            "docs/debug/README.md",
            "docs/debug/debug-directories.md",
            "docs/debug/debug-playbook.md",
            "docs/runtime/runtime-home.md",
            "docs/runtime/runtime-directories.md",
            "docs/config/config-directories.md",
            "docs/design/design-doc-index.md",
            "docs/design/config-core-design.md",
            "docs/design/contracts-core-design.md",
            "docs/design/debug-core-design.md",
            "docs/design/provider-semantic-design.md",
            "docs/design/provider-adapter-design.md",
            "docs/design/reason-turn-design.md",
            "docs/design/reason-persistence-design.md",
            "docs/design/tool-registry-design.md",
            "docs/design/node-master-slave-design.md",
            "docs/design/ui-protocol-design.md",
            "docs/design/runtime-command-dispatch-design.md",
            "docs/design/runtime-daemon-design.md",
            "docs/references/provider-protocols/README.md",
            "docs/references/provider-protocols/openai-responses.md",
            "docs/references/provider-protocols/openai-chat-completions.md",
            "docs/references/provider-protocols/anthropic-messages.md",
            ".agents/skills/freehand-dev/SKILL.md",
            ".agents/skills/freehand-dev/agents/openai.yaml",
            ".agents/skills/provider-protocols/SKILL.md",
            ".agents/skills/provider-protocols/agents/openai.yaml",
            ".githooks/pre-commit",
            ".githooks/pre-push",
            ".github/workflows/ci.yml",
            ".github/workflows/release.yml",
        ],
    )?;
    verify_workspace_members(&root)?;
    verify_skill_rules(&root)?;
    verify_orchestrator_policy_docs(&root)?;
    verify_generated_wiki(&root)?;
    verify_webui_app_boundary(&root)?;
    verify_runtime_daemon_boundary(&root)?;
    Ok(())
}

fn require_files(root: &Path, rel_paths: &[&str]) -> Result<(), String> {
    for rel in rel_paths {
        let path = root.join(rel);
        if !path.is_file() {
            return Err(format!("missing required file: {}", path.display()));
        }
    }
    Ok(())
}

fn verify_workspace_members(root: &Path) -> Result<(), String> {
    let members = [
        "crates/freehand-contracts",
        "crates/freehand-blocks",
        "crates/freehand-config",
        "crates/freehand-provider-core",
        "crates/freehand-provider-openai",
        "crates/freehand-provider-anthropic",
        "crates/freehand-reason",
        "crates/freehand-node",
        "crates/freehand-debug",
        "crates/freehand-ui-protocol",
        "crates/freehand-tools",
        "crates/freehand-runtime",
        "crates/freehand-gates",
        "crates/freehand-testkit",
        "apps/freehand-cli",
        "apps/freehand-server",
        "apps/freehand-daemon",
        "xtask",
    ];
    for member in members {
        let cargo = root.join(member).join("Cargo.toml");
        if !cargo.is_file() {
            return Err(format!(
                "workspace member missing Cargo.toml: {}",
                cargo.display()
            ));
        }
    }
    Ok(())
}

fn verify_skill_rules(root: &Path) -> Result<(), String> {
    let skill = fs::read_to_string(root.join(".agents/skills/freehand-dev/SKILL.md"))
        .map_err(|err| err.to_string())?;
    let required_skill_snippets = [
        "Runtime home is `~/.freehand`.",
        "Start from `feature_id`, owner, `debug_artifacts`, and runtime paths in the function map.",
        "If feature truth changed, update function map, architecture docs, skill workflow, and memory files in the same task.",
        "Before adding any function, inspect existing blocks and owner crates first.",
        "docs/references/provider-protocols/",
        "request mainline",
        "response mainline",
        "function-call tables",
        "Do not add temporary helpers to `crates/freehand-reason` or `crates/freehand-node`.",
        "module white-box tests",
        "module black-box tests",
        "project black-box tests",
        "built-in tool specs and execution ownership live in `crates/freehand-tools`",
        "runtime must not hardcode demo tool schemas or demo tool execution outside `crates/freehand-tools`",
        "no tool may be exposed on the live provider path until its function map and test-design docs are updated in the same change set",
        "test-design record",
        "Owner Routing Index",
        "Owner Routing",
        "cargo build --workspace",
        "cargo run -p xtask -- gates check",
    ];
    for snippet in required_skill_snippets {
        if !skill.contains(snippet) {
            return Err(format!("skill missing required rule: {snippet}"));
        }
    }
    Ok(())
}

fn verify_orchestrator_policy_docs(root: &Path) -> Result<(), String> {
    let files: Vec<(PathBuf, &[&str])> = vec![
        (
            root.join("AGENTS.md"),
            &[
                "This file is the repo entry router.",
                "feature/function owner lookup:",
                "debug starts from `feature_id`, owner, debug artifacts, and runtime directories.",
                "If truth changes, update docs, function map, skill workflow, and memory in same task.",
            ],
        ),
        (
            root.join("docs/architecture/workspace-layout.md"),
            &[
                "Before writing any new function, inspect existing function libraries",
                "freehand-blocks",
                "Function map drives owner lookup and debug entry.",
            ],
        ),
        (
            root.join("docs/architecture/feature-map.md"),
            &[
                "Owner Routing Index",
                "problem area",
                "feature_id",
                "test orchestration",
                "first-version path tools remain locked to one workspace-root policy",
            ],
        ),
        (
            root.join("docs/architecture/function-map-spec.md"),
            &[
                "Temporary helper functions are forbidden in orchestrator crates",
                "freehand-blocks",
                "required_white_box_tests",
                "required_module_black_box_tests",
                "required_project_black_box_tests",
                "test_design_doc",
                "function_map_doc",
                "tool-facing features must not expose a new tool before the function map binds that tool surface and its execution path",
                "request mainline description",
                "function call table",
                "mainline call source",
                "generated wiki",
            ],
        ),
        (
            root.join("docs/function-maps/README.md"),
            &[
                "request mainline",
                "response mainline",
                "error mainline",
                "Shared Multi-Reference Function Rule",
                "bind to code",
                "Owner Routing Rule",
                "test orchestration document",
                "machine-readable mainline call source",
            ],
        ),
        (
            root.join("docs/mainline-calls/README.md"),
            &[
                "machine-readable mainline call",
                "source of truth",
                "generated wiki",
            ],
        ),
        (
            root.join("docs/function-maps/tool.registry.md"),
            &[
                "path-based read-only tools resolve against one locked workspace root",
                "first real read-only execution set is",
                "ExecutionFailed",
                "execute_read_file",
                "execute_glob",
                "execute_grep",
                "execute_ls",
                "docs/mainline-calls/tool.registry.json",
                "docs/wiki/tool.registry.md",
            ],
        ),
        (
            root.join("docs/architecture/test-strategy.md"),
            &[
                "module white-box",
                "module black-box",
                "project black-box",
                "cargo test --workspace",
                "test-design record",
                "test orchestration starts from `feature_id`",
                "runtime no-hardcoded-demo-tool regression",
            ],
        ),
        (
            root.join("docs/testing/tool.registry.md"),
            &[
                "`read_file` line-window and path-lock tests",
                "`glob` recursive and simple-filename pattern tests",
                "`grep` recursive match tests",
                "`ls` flat and recursive listing tests",
                "runtime live bridge can execute a real implemented read-only registry tool and re-enter the result",
                "wiki generated from mainline call",
            ],
        ),
        (
            root.join("docs/design/tool-registry-design.md"),
            &[
                "first real file/search batch is read-only and workspace-locked",
                "Current first implemented set",
                "first-version path tools are directory-locked to the canonical process current working directory",
            ],
        ),
        (
            root.join("docs/runtime/runtime-home.md"),
            &["Freehand runtime home is `~/.freehand`."],
        ),
        (
            root.join("docs/architecture/dev-debug-workflow.md"),
            &[
                "open `feature-map.md`",
                "module white-box",
                "module black-box",
                "project black-box",
                "test-design record",
                "Problem Location Rule",
                "Owner Routing Index",
                "if truth changed, update map/docs/skill/memory in same task",
            ],
        ),
        (
            root.join("docs/debug/debug-directories.md"),
            &["new debug path must be documented before use"],
        ),
        (
            root.join("docs/config/config-directories.md"),
            &["secret values stay out of repo config files"],
        ),
        (
            root.join("docs/design/design-doc-index.md"),
            &[
                "chat discussion is not durable design truth",
                "reason-persistence-design.md",
                "debug-core-design.md",
                "tool-registry-design.md",
            ],
        ),
        (
            root.join("docs/wiki/README.md"),
            &[
                "Generated wiki",
                "mainline call source",
                "Do not edit by hand",
            ],
        ),
        (
            root.join("docs/wiki/tool.registry.md"),
            &[
                "Generated from",
                "tool.registry",
                "read_file",
                "glob",
                "grep",
                "ls",
            ],
        ),
        (
            root.join("docs/design/debug-core-design.md"),
            &[
                "`debug.core` is the independent observation module",
                "does not own request truth",
                "does not own session truth",
                "UI consumes debug state through `freehand-ui-protocol`",
            ],
        ),
        (
            root.join("docs/design/reason-persistence-design.md"),
            &[
                "authoritative snapshots",
                "append-only ledgers",
                "derived UI and index sidecars",
                "provider raw payloads are debug-only artifacts",
            ],
        ),
        (
            root.join("docs/design/config-core-design.md"),
            &[
                "config lives only at `~/.freehand/config.toml`",
                "multi-agent layout uses named tables:",
                "[agents.<name>]",
            ],
        ),
        (
            root.join("docs/design/contracts-core-design.md"),
            &[
                "`contracts.core` covers cross-module shared semantic types.",
                "serializable",
                "replayable",
                "persistable",
            ],
        ),
        (
            root.join("docs/design/provider-semantic-design.md"),
            &[
                "OpenAI-compatible providers",
                "Anthropic providers",
                "period unit is seconds",
                "raw provider events are retained in debug mode",
            ],
        ),
        (
            root.join("docs/design/reason-turn-design.md"),
            &[
                "turn truth is stored per turn",
                "only `freehand-reason` may write session truth",
                "provider `finish_reason=stop` or `finish_reason=end_turn` does not by itself stop Freehand turn execution",
            ],
        ),
        (
            root.join("docs/design/node-master-slave-design.md"),
            &[
                "one local `master`",
                "one local `slave`",
                "pair through WebSocket handshake",
                "continues listening",
            ],
        ),
        (
            root.join("docs/design/ui-protocol-design.md"),
            &[
                "First version supports:",
                "CLI",
                "WebUI",
                "query and subscribe are separate",
                "`source_agent_id`",
                "input ingress plus read-only projection boundary",
            ],
        ),
        (
            root.join("docs/design/ui-and-runtime-topology.md"),
            &[
                "input port plus a read-only consumer of reason/debug projections",
                "must not directly write reason truth",
                "must not directly write debug truth",
            ],
        ),
        (
            root.join("docs/references/provider-protocols/README.md"),
            &[
                "official provider documentation",
                "OpenAI Responses API",
                "Anthropic Messages API",
            ],
        ),
    ];
    for (file, required) in files {
        let text = fs::read_to_string(&file).map_err(|err| err.to_string())?;
        for snippet in required {
            if !text.contains(snippet) {
                return Err(format!(
                    "policy doc missing required snippet `{snippet}` in {}",
                    file.display()
                ));
            }
        }
    }
    Ok(())
}

fn verify_generated_wiki(root: &Path) -> Result<(), String> {
    let generated = render_all_mainline_wikis(root)?;
    for (path, expected) in generated {
        let actual = fs::read_to_string(&path)
            .map_err(|err| format!("read generated wiki {}: {err}", path.display()))?;
        if actual != expected {
            return Err(format!(
                "generated wiki out of date: {}\nrun `cargo run -p xtask -- mainlines generate`",
                path.display()
            ));
        }
    }
    Ok(())
}

fn generate_mainline_wikis(root: &Path, write: bool) -> Result<(), String> {
    let generated = render_all_mainline_wikis(root)?;
    for (path, content) in generated {
        if write {
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|err| err.to_string())?;
            }
            fs::write(&path, content).map_err(|err| err.to_string())?;
        } else {
            let actual = fs::read_to_string(&path)
                .map_err(|err| format!("read generated wiki {}: {err}", path.display()))?;
            if actual != content {
                return Err(format!(
                    "generated wiki out of date: {}\nrun `cargo run -p xtask -- mainlines generate`",
                    path.display()
                ));
            }
        }
    }
    Ok(())
}

fn render_all_mainline_wikis(root: &Path) -> Result<Vec<(PathBuf, String)>, String> {
    let docs_dir = root.join("docs/mainline-calls");
    let mut source_paths = Vec::new();
    for entry in fs::read_dir(&docs_dir).map_err(|err| err.to_string())? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("json") {
            source_paths.push(path);
        }
    }
    source_paths.sort();

    let mut generated = Vec::new();
    let mut wiki_index = String::new();
    wiki_index.push_str("# Generated Wiki Index\n\n");
    wiki_index.push_str(
        "Generated wiki artifacts from machine-readable mainline call source files. Do not edit by hand.\n\n",
    );
    for source_path in source_paths {
        let doc = load_mainline_doc(&source_path)?;
        let wiki_path = root.join(&doc.generated_wiki_doc);
        let wiki_content = render_mainline_wiki(&doc);
        wiki_index.push_str(&format!(
            "- [{}](./{}.md) mainline call source `{}`\n",
            doc.feature_id, doc.feature_id, doc.mainline_call_doc
        ));
        generated.push((wiki_path, wiki_content));
    }
    generated.push((root.join("docs/wiki/README.md"), wiki_index));
    Ok(generated)
}

fn load_mainline_doc(path: &Path) -> Result<MainlineCallDoc, String> {
    let text = fs::read_to_string(path)
        .map_err(|err| format!("read mainline call source {}: {err}", path.display()))?;
    serde_json::from_str(&text)
        .map_err(|err| format!("parse mainline call source {}: {err}", path.display()))
}

fn render_mainline_wiki(doc: &MainlineCallDoc) -> String {
    let mut out = String::new();
    out.push_str(&format!("# Wiki: `{}`\n\n", doc.feature_id));
    out.push_str(&format!(
        "Generated from `{}`. Do not edit by hand.\n\n",
        doc.mainline_call_doc
    ));
    out.push_str(&format!("- owner crate: `{}`\n", doc.owner_crate));
    out.push_str(&format!("- owner module: `{}`\n", doc.owner_module));
    out.push_str(&format!("- function map: `{}`\n", doc.function_map_doc));
    out.push_str(&format!("- generated wiki: `{}`\n", doc.generated_wiki_doc));
    out.push_str(&format!("- test design: `{}`\n\n", doc.test_design_doc));
    render_bullets(&mut out, "Request Mainline", &doc.request_mainline);
    render_bullets(&mut out, "Response Mainline", &doc.response_mainline);
    render_bullets(&mut out, "Error Mainline", &doc.error_mainline);
    out.push_str("## Shared Multi-Reference Functions\n\n");
    for shared in &doc.shared_functions {
        out.push_str(&format!("- `{}`\n", shared.symbol));
        out.push_str(&format!("  - owner: `{}`\n", shared.owner));
        out.push_str(&format!("  - purpose: {}\n", shared.purpose));
        out.push_str(&format!(
            "  - allowed callers: {}\n",
            shared.allowed_callers.join(", ")
        ));
        out.push_str(&format!(
            "  - related tests: {}\n",
            shared.related_tests.join(", ")
        ));
        out.push_str(&format!("  - why shared: {}\n", shared.why_shared));
    }
    out.push_str("\n## Function Call Table\n\n");
    out.push_str("| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | binding status |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for row in &doc.call_table {
        out.push_str(&format!(
            "| {} | `{}` | `{}` | {} | {} | {} | {} | {} | {} |\n",
            row.step,
            row.symbol_path,
            row.file_path,
            row.responsibility,
            row.input_semantic,
            row.output_semantic,
            row.caller,
            row.callee,
            row.binding_status,
        ));
    }
    out.push_str("\n## Sync Status Against Mainline Call\n\n");
    for line in &doc.sync_status {
        out.push_str(&format!("- {}\n", line));
    }
    out
}

fn render_bullets(out: &mut String, title: &str, items: &[String]) {
    out.push_str(&format!("## {}\n\n", title));
    for item in items {
        out.push_str(&format!("- {}\n", item));
    }
    out.push('\n');
}

#[derive(Debug, Deserialize, Serialize)]
struct MainlineCallDoc {
    feature_id: String,
    owner_crate: String,
    owner_module: String,
    function_map_doc: String,
    test_design_doc: String,
    mainline_call_doc: String,
    generated_wiki_doc: String,
    request_mainline: Vec<String>,
    response_mainline: Vec<String>,
    error_mainline: Vec<String>,
    shared_functions: Vec<SharedMainlineFunction>,
    call_table: Vec<MainlineCallRow>,
    sync_status: Vec<String>,
}

#[derive(Debug, Deserialize, Serialize)]
struct SharedMainlineFunction {
    symbol: String,
    owner: String,
    purpose: String,
    allowed_callers: Vec<String>,
    related_tests: Vec<String>,
    why_shared: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct MainlineCallRow {
    step: String,
    symbol_path: String,
    file_path: String,
    responsibility: String,
    input_semantic: String,
    output_semantic: String,
    caller: String,
    callee: String,
    binding_status: String,
}

fn verify_webui_app_boundary(root: &Path) -> Result<(), String> {
    let cargo = fs::read_to_string(root.join("apps/freehand-server/Cargo.toml"))
        .map_err(|err| err.to_string())?;
    let forbidden = [
        "freehand-config",
        "freehand-node",
        "freehand-provider-core",
        "freehand-provider-openai",
        "freehand-provider-anthropic",
        "freehand-reason",
    ];
    for crate_name in forbidden {
        if cargo.contains(crate_name) {
            return Err(format!(
                "freehand-server must stay protocol-only and must not depend on {crate_name}"
            ));
        }
    }
    Ok(())
}

fn verify_runtime_daemon_boundary(root: &Path) -> Result<(), String> {
    let cargo = fs::read_to_string(root.join("apps/freehand-daemon/Cargo.toml"))
        .map_err(|err| err.to_string())?;
    for required in ["freehand-runtime", "freehand-server"] {
        if !cargo.contains(required) {
            return Err(format!(
                "freehand-daemon must depend on {required} for runtime-host transport injection"
            ));
        }
    }
    let forbidden = [
        "freehand-config",
        "freehand-node",
        "freehand-provider-core",
        "freehand-provider-openai",
        "freehand-provider-anthropic",
        "freehand-reason",
    ];
    for crate_name in forbidden {
        if cargo.contains(crate_name) {
            return Err(format!(
                "freehand-daemon must depend on freehand-runtime, not directly on {crate_name}"
            ));
        }
    }
    Ok(())
}
