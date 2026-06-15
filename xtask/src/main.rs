use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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
        _ => {
            eprintln!("usage: cargo run -p xtask -- gates check");
            std::process::exit(1);
        }
    }
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
            "docs/function-maps/foundation.workspace.md",
            "docs/function-maps/config.core.md",
            "docs/function-maps/provider.semantic.md",
            "docs/function-maps/contracts.core.md",
            "docs/function-maps/reason.turn.md",
            "docs/function-maps/ui.protocol.md",
            "docs/function-maps/node.master-slave.md",
            "docs/architecture/debug-and-trace.md",
            "docs/architecture/dev-gates.md",
            "docs/architecture/dev-debug-workflow.md",
            "docs/architecture/test-strategy.md",
            "docs/testing/foundation.workspace.md",
            "docs/testing/config.core.md",
            "docs/testing/provider.semantic.md",
            "docs/testing/contracts.core.md",
            "docs/testing/reason.turn.md",
            "docs/testing/ui.protocol.md",
            "docs/testing/node.master-slave.md",
            "docs/debug/README.md",
            "docs/debug/debug-directories.md",
            "docs/debug/debug-playbook.md",
            "docs/runtime/runtime-home.md",
            "docs/runtime/runtime-directories.md",
            "docs/config/config-directories.md",
            "docs/design/design-doc-index.md",
            "docs/design/config-core-design.md",
            "docs/design/contracts-core-design.md",
            "docs/design/provider-semantic-design.md",
            "docs/design/reason-turn-design.md",
            "docs/design/node-master-slave-design.md",
            "docs/design/ui-protocol-design.md",
            "docs/references/provider-protocols/README.md",
            "docs/references/provider-protocols/openai-responses.md",
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
        "crates/freehand-ui-protocol",
        "crates/freehand-gates",
        "crates/freehand-testkit",
        "apps/freehand-cli",
        "apps/freehand-server",
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
        "test-design record",
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
            root.join("docs/architecture/function-map-spec.md"),
            &[
                "Temporary helper functions are forbidden in orchestrator crates",
                "freehand-blocks",
                "required_white_box_tests",
                "required_module_black_box_tests",
                "required_project_black_box_tests",
                "test_design_doc",
                "function_map_doc",
                "request mainline description",
                "function call table",
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
            &["chat discussion is not durable design truth"],
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
