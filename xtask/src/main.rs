use std::collections::{BTreeMap, BTreeSet};
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
            ".ignore",
            "CACHE.md",
            "MEMORY.md",
            "note.md",
            "docs/architecture/feature-map.md",
            "docs/architecture/function-map-spec.md",
            "docs/function-maps/README.md",
            "docs/mainline-calls/README.md",
            "docs/resource-maps/README.md",
            "docs/resource-maps/core.json",
            "docs/function-maps/foundation.workspace.md",
            "docs/function-maps/config.core.md",
            "docs/function-maps/provider.semantic.md",
            "docs/function-maps/provider.openai-adapter.md",
            "docs/function-maps/provider.anthropic-adapter.md",
            "docs/function-maps/tool.registry.md",
            "docs/function-maps/tool.preview.md",
            "docs/function-maps/task.orchestration.md",
            "docs/function-maps/contracts.core.md",
            "docs/function-maps/debug.core.md",
            "docs/function-maps/instruction.capability-loader.md",
            "docs/function-maps/metadata.core.md",
            "docs/function-maps/reason.turn.md",
            "docs/function-maps/reason.session-history.md",
            "docs/function-maps/reason.persistence.md",
            "docs/function-maps/reason.rewrite-policy.md",
            "docs/function-maps/reason.context-planner.md",
            "docs/function-maps/ui.protocol.md",
            "docs/function-maps/node.master-slave.md",
            "docs/function-maps/runtime.ui-command-dispatch.md",
            "docs/function-maps/runtime.checkpoint-rewind.md",
            "docs/function-maps/app.runtime-daemon.md",
            "docs/function-maps/app.cli-runtime-smoke.md",
            "docs/function-maps/app.cli-live-turn.md",
            "docs/function-maps/app.webui-smoke.md",
            "docs/function-maps/app.runtime-daemon.md",
            "docs/mainline-calls/provider.anthropic-adapter.json",
            "docs/mainline-calls/provider.openai-adapter.json",
            "docs/mainline-calls/provider.semantic.json",
            "docs/mainline-calls/provider.reason-live-bridge.json",
            "docs/mainline-calls/tool.registry.json",
            "docs/mainline-calls/tool.preview.json",
            "docs/mainline-calls/task.orchestration.json",
            "docs/mainline-calls/ui.protocol.json",
            "docs/mainline-calls/foundation.workspace.json",
            "docs/mainline-calls/config.core.json",
            "docs/mainline-calls/contracts.core.json",
            "docs/mainline-calls/metadata.core.json",
            "docs/mainline-calls/node.master-slave.json",
            "docs/mainline-calls/app.cli-runtime-smoke.json",
            "docs/mainline-calls/app.cli-live-turn.json",
            "docs/mainline-calls/app.webui-smoke.json",
            "docs/mainline-calls/app.runtime-daemon.json",
            "docs/mainline-calls/debug.core.json",
            "docs/mainline-calls/instruction.capability-loader.json",
            "docs/mainline-calls/reason.turn.json",
            "docs/mainline-calls/reason.session-history.json",
            "docs/mainline-calls/reason.persistence.json",
            "docs/mainline-calls/reason.rewrite-policy.json",
            "docs/mainline-calls/reason.context-planner.json",
            "docs/mainline-calls/runtime.ui-command-dispatch.json",
            "docs/mainline-calls/runtime.checkpoint-rewind.json",
            "docs/wiki/README.md",
            "docs/wiki/provider.anthropic-adapter.md",
            "docs/wiki/provider.openai-adapter.md",
            "docs/wiki/provider.semantic.md",
            "docs/wiki/provider.reason-live-bridge.md",
            "docs/wiki/tool.registry.md",
            "docs/wiki/tool.preview.md",
            "docs/wiki/task.orchestration.md",
            "docs/wiki/ui.protocol.md",
            "docs/wiki/foundation.workspace.md",
            "docs/wiki/config.core.md",
            "docs/wiki/contracts.core.md",
            "docs/wiki/metadata.core.md",
            "docs/wiki/node.master-slave.md",
            "docs/wiki/app.cli-runtime-smoke.md",
            "docs/wiki/app.cli-live-turn.md",
            "docs/wiki/app.webui-smoke.md",
            "docs/wiki/app.runtime-daemon.md",
            "docs/wiki/debug.core.md",
            "docs/wiki/instruction.capability-loader.md",
            "docs/wiki/reason.turn.md",
            "docs/wiki/reason.session-history.md",
            "docs/wiki/reason.persistence.md",
            "docs/wiki/reason.rewrite-policy.md",
            "docs/wiki/reason.context-planner.md",
            "docs/wiki/runtime.ui-command-dispatch.md",
            "docs/wiki/runtime.checkpoint-rewind.md",
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
            "docs/testing/tool.preview.md",
            "docs/testing/task.orchestration.md",
            "docs/testing/contracts.core.md",
            "docs/testing/debug.core.md",
            "docs/testing/instruction.capability-loader.md",
            "docs/testing/metadata.core.md",
            "docs/testing/reason.turn.md",
            "docs/testing/reason.session-history.md",
            "docs/testing/reason.persistence.md",
            "docs/testing/reason.rewrite-policy.md",
            "docs/testing/reason.context-planner.md",
            "docs/testing/ui.protocol.md",
            "docs/testing/node.master-slave.md",
            "docs/testing/runtime.ui-command-dispatch.md",
            "docs/testing/runtime.checkpoint-rewind.md",
            "docs/testing/app.runtime-daemon.md",
            "docs/testing/app.cli-runtime-smoke.md",
            "docs/testing/app.cli-live-turn.md",
            "docs/testing/app.webui-smoke.md",
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
            "docs/design/metadata-core-design.md",
            "docs/design/instruction-capability-loader-design.md",
            "docs/design/provider-semantic-design.md",
            "docs/design/provider-adapter-design.md",
            "docs/design/reason-turn-design.md",
            "docs/design/reason-persistence-design.md",
            "docs/design/tool-registry-design.md",
            "docs/design/tool-preview-design.md",
            "docs/design/task-orchestration-design.md",
            "docs/design/node-master-slave-design.md",
            "docs/design/ui-protocol-design.md",
            "docs/design/runtime-command-dispatch-design.md",
            "docs/design/runtime-checkpoint-rewind-design.md",
            "docs/design/runtime-daemon-design.md",
            "docs/references/provider-protocols/README.md",
            "docs/references/provider-protocols/openai-responses.md",
            "docs/references/provider-protocols/openai-chat-completions.md",
            "docs/references/provider-protocols/anthropic-messages.md",
            "scripts/source-search.sh",
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
    verify_feature_map_unique_entries(&root)?;
    verify_resource_map(&root)?;
    verify_generated_wiki(&root)?;
    verify_mainline_manifest_links(&root)?;
    verify_mainline_call_table_bindings(&root)?;
    verify_ci_cd_gate_commands(&root)?;
    verify_source_search_policy(&root)?;
    verify_data_control_boundaries(&root)?;
    verify_webui_app_boundary(&root)?;
    verify_runtime_daemon_boundary(&root)?;
    verify_dependency_graph(&root)?;
    verify_task_status_single_writer(&root)?;
    verify_adp_protocol_artifacts(&root)?;
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
        "crates/freehand-metadata",
        "crates/freehand-instructions",
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
        "Read `docs/resource-maps/core.json`.",
        "Identify the source resource, target resource, and whether the relation is direct or indirect.",
        "If feature truth changed, update resource map, function map, architecture docs, skill workflow, and memory files in the same task.",
        "Before adding any function, inspect existing blocks and owner crates first.",
        "docs/references/provider-protocols/",
        "request mainline",
        "response mainline",
        "function-call tables",
        "compiled manifests",
        "resolvable symbols",
        "crates/freehand-metadata",
        "writer owner and write-node provenance",
        "Control semantics must be extracted from data pipelines",
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
        "cargo run -p xtask -- mainlines check",
        "cargo run -p xtask -- gates check",
        "CI/CD command alignment",
        "make ci",
        "scripts/source-search.sh",
        "Do not search generated or runtime output when locating implementation truth",
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
                "resource-map-first ownership",
                "feature/function owner lookup:",
                "debug starts from `resource_type`, allowed resource relation, `feature_id`, owner, debug artifacts, and runtime directories.",
                "If truth changes, update resource map, docs, function map, skill workflow, and memory in same task.",
            ],
        ),
        (
            root.join("docs/architecture/workspace-layout.md"),
            &[
                "Before writing any new function, inspect existing function libraries",
                "freehand-blocks",
                "Function map drives owner lookup and debug entry.",
                "freehand-metadata",
                "Control semantics must be extracted from data pipelines.",
            ],
        ),
        (
            root.join("docs/architecture/feature-map.md"),
            &[
                "Owner Routing Index",
                "problem area",
                "feature_id",
                "test orchestration",
                "metadata.core",
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
                "data/control isolation notes",
                "metadata owner/write-node notes",
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
                "where metadata writes route through `metadata.core`",
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
            root.join("docs/architecture/dev-gates.md"),
            &[
                "Mainline Manifest Gate",
                "deterministic manifests",
                "function_map_doc",
                "test_design_doc",
                "generated_wiki_doc",
                "compiled review surfaces",
                "Mainline Call-Table Binding Gate",
                "binding_status = \"bound\"",
                "symbol_path",
                "CI/CD Command Alignment Gate",
                "make ci",
                "cargo run -p xtask -- mainlines check",
                "data/control separation",
                "metadata/debug/control owner types",
            ],
        ),
        (
            root.join("docs/function-maps/tool.registry.md"),
            &[
                "the master-safe export excludes unrestricted shell scope",
                "read-only path tools remain locked to the current workspace root",
                "external absolute paths are explicit",
                "file-mutation tools remain locked to the current workspace root",
                "WorkspaceBoundaryViolation",
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
                "`read_file` line-window and external absolute rejection tests",
                "`glob` recursive and simple-filename pattern tests",
                "`grep` recursive match and external absolute rejection tests",
                "`ls` flat, recursive listing, and external absolute rejection tests",
                "read-only path tools reject existing external absolute paths",
                "worker read-only path tools and file-mutation tools remain locked",
                "wiki generated from mainline call",
            ],
        ),
        (
            root.join("docs/design/tool-registry-design.md"),
            &[
                "first real file/search batch is read-only",
                "Current first implemented set",
                "read-only path tools are locked to the current workspace root",
                "unrestricted shell is not exposed to Worker provider turns",
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
                "metadata-core-design.md",
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
            root.join("docs/design/metadata-core-design.md"),
            &[
                "`metadata.core` is the central owner for internal control/provenance metadata.",
                "Every metadata envelope must include:",
                "`MetadataWriteOwner.feature_id`",
                "`MetadataWriteNode.pipeline_node`",
                "Metadata is not request data.",
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

fn verify_feature_map_unique_entries(root: &Path) -> Result<(), String> {
    let feature_map_path = root.join("docs/architecture/feature-map.md");
    let feature_map = fs::read_to_string(&feature_map_path)
        .map_err(|err| format!("read feature map {}: {err}", feature_map_path.display()))?;
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();

    for line in feature_map.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with("### `") || !trimmed.ends_with('`') {
            continue;
        }
        let feature_id = &trimmed[5..trimmed.len() - 1];
        *counts.entry(feature_id.to_owned()).or_default() += 1;
    }

    if let Some((feature_id, count)) = counts.into_iter().find(|(_, count)| *count > 1) {
        return Err(format!(
            "feature map duplicate seed entry for `{feature_id}` in {} ({count} entries)",
            feature_map_path.display()
        ));
    }

    Ok(())
}

fn verify_resource_map(root: &Path) -> Result<(), String> {
    let path = root.join("docs/resource-maps/core.json");
    let text = fs::read_to_string(&path)
        .map_err(|err| format!("read resource map {}: {err}", path.display()))?;
    let map: ResourceMapDoc = serde_json::from_str(&text)
        .map_err(|err| format!("parse resource map {}: {err}", path.display()))?;
    if map.schema_version == 0 {
        return Err("resource map schema_version must be positive".to_owned());
    }

    let feature_map_path = root.join("docs/architecture/feature-map.md");
    let feature_map = fs::read_to_string(&feature_map_path)
        .map_err(|err| format!("read feature map {}: {err}", feature_map_path.display()))?;

    let mut resources = BTreeSet::new();
    let mut resource_owner_lookup: BTreeMap<String, String> = BTreeMap::new();
    let mut resource_operations: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let feature_resource_ownership = parse_feature_map_resource_ownership(&feature_map)?;
    let feature_seed_owners = parse_feature_map_seed_owners(&feature_map)?;
    for resource in &map.resources {
        require_non_empty(&resource.resource_type, "resource_type")?;
        require_non_empty(&resource.owner_feature_id, "owner_feature_id")?;
        require_non_empty(&resource.owner_crate, "owner_crate")?;
        require_non_empty(&resource.identity, "identity")?;
        require_non_empty(&resource.truth_store, "truth_store")?;
        if resource.operations.is_empty() {
            return Err(format!(
                "resource `{}` must declare at least one operation",
                resource.resource_type
            ));
        }
        if resource.projections.is_empty() {
            return Err(format!(
                "resource `{}` must declare at least one projection",
                resource.resource_type
            ));
        }
        let mut operations = BTreeSet::new();
        for operation in &resource.operations {
            require_non_empty(operation, "resource.operations")?;
            if !operations.insert(operation.clone()) {
                return Err(format!(
                    "resource `{}` has duplicate operation `{}`",
                    resource.resource_type, operation
                ));
            }
        }
        let mut projections = BTreeSet::new();
        for projection in &resource.projections {
            require_non_empty(projection, "resource.projections")?;
            if !projections.insert(projection.clone()) {
                return Err(format!(
                    "resource `{}` has duplicate projection `{}`",
                    resource.resource_type, projection
                ));
            }
        }
        if !resources.insert(resource.resource_type.clone()) {
            return Err(format!(
                "duplicate resource_type `{}` in docs/resource-maps/core.json",
                resource.resource_type
            ));
        }
        resource_operations.insert(resource.resource_type.clone(), operations);
        resource_owner_lookup.insert(
            resource.resource_type.clone(),
            resource.owner_feature_id.clone(),
        );
        let feature_marker = format!("`{}`", resource.owner_feature_id);
        require_contains(
            &feature_map,
            &feature_marker,
            "docs/architecture/feature-map.md",
        )?;
        let feature_owner = feature_seed_owners
            .get(resource.owner_feature_id.as_str())
            .ok_or_else(|| {
                format!(
                    "feature map missing seed owner for `{}`",
                    resource.owner_feature_id
                )
            })?;
        if !feature_owner.contains(&resource.owner_crate) {
            return Err(format!(
                "resource `{}` owner_crate `{}` is not present in feature map owner `{}` for `{}`",
                resource.resource_type,
                resource.owner_crate,
                feature_owner,
                resource.owner_feature_id
            ));
        }
        let owned_resources = feature_resource_ownership
            .get(resource.owner_feature_id.as_str())
            .ok_or_else(|| {
                format!(
                    "feature map Resource Ownership Index missing owner feature `{}` for resource `{}`",
                    resource.owner_feature_id, resource.resource_type
                )
            })?;
        if !owned_resources.contains(resource.resource_type.as_str()) {
            return Err(format!(
                "feature map Resource Ownership Index owner `{}` does not list resource `{}`",
                resource.owner_feature_id, resource.resource_type
            ));
        }
    }
    let mut feature_index_resource_owners = BTreeMap::new();
    for (feature_id, owned_resources) in &feature_resource_ownership {
        for resource_type in owned_resources {
            if !resources.contains(resource_type.as_str()) {
                return Err(format!(
                    "feature map Resource Ownership Index feature `{feature_id}` lists unknown resource `{resource_type}`"
                ));
            }
            if let Some(previous_owner) =
                feature_index_resource_owners.insert(resource_type.clone(), feature_id.clone())
            {
                return Err(format!(
                    "feature map Resource Ownership Index resource `{resource_type}` is owned by both `{previous_owner}` and `{feature_id}`"
                ));
            }
            let expected_owner = resource_owner_lookup.get(resource_type).ok_or_else(|| {
                format!(
                    "resource owner lookup missing `{resource_type}` while validating feature map"
                )
            })?;
            if expected_owner != feature_id {
                return Err(format!(
                    "feature map Resource Ownership Index resource `{resource_type}` is listed under `{feature_id}` but resource map owner is `{expected_owner}`"
                ));
            }
        }
    }
    if map.resource_map_id == "freehand.core-resource-map" {
        verify_required_core_resources(&resources)?;
    }

    let mut operation_ids = BTreeSet::new();
    let mut operation_lookup: BTreeMap<String, (&str, &str)> = BTreeMap::new();
    let mut operation_binding_lookup: BTreeMap<&str, &ResourceMapOperationBinding> =
        BTreeMap::new();
    let mut operation_pairs = BTreeSet::new();
    let mut bound_operation_pairs = BTreeSet::new();
    for binding in &map.operation_bindings {
        require_non_empty(&binding.operation_id, "operation_bindings.operation_id")?;
        require_non_empty(
            &binding.owner_feature_id,
            "operation_bindings.owner_feature_id",
        )?;
        require_non_empty(
            &binding.source_resource,
            "operation_bindings.source_resource",
        )?;
        require_non_empty(
            &binding.target_resource,
            "operation_bindings.target_resource",
        )?;
        require_non_empty(&binding.effect, "operation_bindings.effect")?;
        require_non_empty(
            &binding.mainline_call_doc,
            "operation_bindings.mainline_call_doc",
        )?;
        require_non_empty(&binding.binding_status, "operation_bindings.binding_status")?;
        if !operation_ids.insert(binding.operation_id.clone()) {
            return Err(format!(
                "duplicate resource operation_id `{}` in docs/resource-maps/core.json",
                binding.operation_id
            ));
        }
        operation_binding_lookup.insert(binding.operation_id.as_str(), binding);
        operation_lookup.insert(
            binding.operation_id.clone(),
            (&binding.source_resource, &binding.target_resource),
        );
        operation_pairs.insert((
            binding.source_resource.clone(),
            binding.target_resource.clone(),
        ));
        let (operation_source, operation_name) =
            binding.operation_id.split_once('.').ok_or_else(|| {
                format!(
                    "resource operation `{}` must use `<source_resource>.<operation>` format",
                    binding.operation_id
                )
            })?;
        if operation_source != binding.source_resource {
            return Err(format!(
                "resource operation `{}` source prefix `{}` does not match source_resource `{}`",
                binding.operation_id, operation_source, binding.source_resource
            ));
        }
        let allowed_operations = resource_operations
            .get(&binding.source_resource)
            .ok_or_else(|| {
                format!(
                    "resource operation `{}` references source resource without operations `{}`",
                    binding.operation_id, binding.source_resource
                )
            })?;
        if !allowed_operations.contains(operation_name) {
            return Err(format!(
                "resource operation `{}` is not declared in resource `{}` operations",
                binding.operation_id, binding.source_resource
            ));
        }
        if binding.binding_status == "bound" {
            bound_operation_pairs.insert((
                binding.source_resource.clone(),
                binding.target_resource.clone(),
            ));
        }
        require_known_resource(&resources, &binding.source_resource, &binding.operation_id)?;
        require_known_resource(&resources, &binding.target_resource, &binding.operation_id)?;
        if !root.join(&binding.mainline_call_doc).is_file() {
            return Err(format!(
                "resource operation `{}` references missing mainline call source `{}`",
                binding.operation_id, binding.mainline_call_doc
            ));
        }
        let feature_marker = format!("`{}`", binding.owner_feature_id);
        require_contains(
            &feature_map,
            &feature_marker,
            "docs/architecture/feature-map.md",
        )?;
        match binding.binding_status.as_str() {
            "bound" => {}
            "pending" => {
                require_non_empty(
                    binding.pending_reason.as_deref().unwrap_or_default(),
                    "operation_bindings.pending_reason",
                )?;
                require_non_empty(
                    binding.pending_closure_doc.as_deref().unwrap_or_default(),
                    "operation_bindings.pending_closure_doc",
                )?;
                require_non_empty(
                    binding.pending_verification.as_deref().unwrap_or_default(),
                    "operation_bindings.pending_verification",
                )?;
                if !root
                    .join(binding.pending_closure_doc.as_deref().unwrap_or_default())
                    .is_file()
                {
                    return Err(format!(
                        "pending resource operation `{}` references missing pending_closure_doc `{}`",
                        binding.operation_id,
                        binding.pending_closure_doc.as_deref().unwrap_or_default()
                    ));
                }
            }
            status => {
                return Err(format!(
                    "resource operation `{}` has unsupported binding_status `{}`",
                    binding.operation_id, status
                ));
            }
        }
        let mainline_path = root.join(&binding.mainline_call_doc);
        let mainline = load_mainline_doc(&mainline_path)?;
        require_equal(
            &mainline.feature_id,
            &binding.owner_feature_id,
            &binding.operation_id,
            "resource operation owner_feature_id",
        )?;
        if !mainline.resource_operations.contains(&binding.operation_id) {
            return Err(format!(
                "resource operation `{}` is not backlinked from `{}` resource_operations",
                binding.operation_id, binding.mainline_call_doc
            ));
        }
        let has_call_row_backlink = mainline
            .call_table
            .iter()
            .any(|row| row.resource_operation.as_deref() == Some(binding.operation_id.as_str()));
        if binding.binding_status == "bound" && !has_call_row_backlink {
            return Err(format!(
                "bound resource operation `{}` is not backlinked from any call_table row in `{}`",
                binding.operation_id, binding.mainline_call_doc
            ));
        }
        let function_map_path = root.join(&mainline.function_map_doc);
        let function_map = fs::read_to_string(&function_map_path).map_err(|err| {
            format!(
                "read function map {} for resource operation `{}`: {err}",
                function_map_path.display(),
                binding.operation_id
            )
        })?;
        require_contains(
            &function_map,
            "docs/resource-maps/core.json",
            &mainline.function_map_doc,
        )?;
        if !mainline.resource_operations.is_empty() {
            let resource_binding_section =
                resource_map_binding_section(&function_map, &mainline.function_map_doc)?;
            require_function_map_binding_label_has_value(
                resource_binding_section,
                "owned resources",
                &mainline.function_map_doc,
            )?;
            require_function_map_binding_label_has_value(
                resource_binding_section,
                "touched resources",
                &mainline.function_map_doc,
            )?;
            require_function_map_binding_label_has_value(
                resource_binding_section,
                "resource operations",
                &mainline.function_map_doc,
            )?;
            require_function_map_binding_label_has_value(
                resource_binding_section,
                "forbidden shortcuts",
                &mainline.function_map_doc,
            )?;
            require_contains(
                resource_binding_section,
                &format!("`{}`", binding.source_resource),
                &mainline.function_map_doc,
            )?;
            require_contains(
                resource_binding_section,
                &format!("`{}`", binding.target_resource),
                &mainline.function_map_doc,
            )?;
            require_contains(
                resource_binding_section,
                &binding.operation_id,
                &mainline.function_map_doc,
            )?;
        }
        require_contains(
            &function_map,
            &binding.operation_id,
            &mainline.function_map_doc,
        )?;
        let test_design_path = root.join(&mainline.test_design_doc);
        let test_design = fs::read_to_string(&test_design_path).map_err(|err| {
            format!(
                "read test design {} for resource operation `{}`: {err}",
                test_design_path.display(),
                binding.operation_id
            )
        })?;
        require_contains(
            &test_design,
            "docs/resource-maps/core.json",
            &mainline.test_design_doc,
        )?;
        require_contains(
            &test_design,
            &binding.operation_id,
            &mainline.test_design_doc,
        )?;
        require_resource_operation_test_coverage(
            root,
            &test_design,
            &binding.operation_id,
            &binding.binding_status,
            &mainline.test_design_doc,
        )?;
    }

    let mut source_edge_ids = BTreeSet::new();
    let mut source_edge_registry = BTreeSet::new();
    for edge in &map.source_edge_registry {
        require_non_empty(&edge.edge_id, "source_edge_registry.edge_id")?;
        require_non_empty(&edge.operation_id, "source_edge_registry.operation_id")?;
        require_known_resource(&resources, &edge.source_resource, &edge.edge_id)?;
        require_known_resource(&resources, &edge.target_resource, &edge.edge_id)?;
        require_non_empty(
            &edge.mainline_call_doc,
            "source_edge_registry.mainline_call_doc",
        )?;
        require_non_empty(
            &edge.call_table_step,
            "source_edge_registry.call_table_step",
        )?;
        require_non_empty(&edge.file_path, "source_edge_registry.file_path")?;
        require_non_empty(&edge.symbol_path, "source_edge_registry.symbol_path")?;
        let edge_file_paths = split_binding_segments(&edge.file_path);
        let edge_symbol_paths = split_binding_segments(&edge.symbol_path);
        if edge_file_paths.is_empty() {
            return Err(format!(
                "source_edge_registry `{}` has no file_path binding",
                edge.edge_id
            ));
        }
        if edge_symbol_paths.is_empty() {
            return Err(format!(
                "source_edge_registry `{}` has no symbol_path binding",
                edge.edge_id
            ));
        }
        for file_path in &edge_file_paths {
            if !root.join(file_path).is_file() {
                return Err(format!(
                    "source_edge_registry `{}` references missing file `{}`",
                    edge.edge_id, file_path
                ));
            }
        }
        for symbol_path in &edge_symbol_paths {
            if !symbol_resolves_in_files(root, &edge_file_paths, symbol_path)? {
                return Err(format!(
                    "source_edge_registry `{}` references missing symbol `{}` in `{}`",
                    edge.edge_id, symbol_path, edge.file_path
                ));
            }
        }
        if !source_edge_ids.insert(edge.edge_id.clone()) {
            return Err(format!(
                "duplicate source_edge_registry edge_id `{}` in docs/resource-maps/core.json",
                edge.edge_id
            ));
        }
        let binding = operation_binding_lookup
            .get(edge.operation_id.as_str())
            .ok_or_else(|| {
                format!(
                    "source_edge_registry `{}` references unknown operation_id `{}`",
                    edge.edge_id, edge.operation_id
                )
            })?;
        if binding.binding_status != "bound" {
            return Err(format!(
                "source_edge_registry `{}` references non-bound operation `{}`",
                edge.edge_id, edge.operation_id
            ));
        }
        require_equal(
            &edge.binding_status,
            &binding.binding_status,
            &edge.edge_id,
            "source_edge_registry binding_status",
        )?;
        require_equal(
            &edge.source_resource,
            &binding.source_resource,
            &edge.edge_id,
            "source_edge_registry source_resource",
        )?;
        require_equal(
            &edge.target_resource,
            &binding.target_resource,
            &edge.edge_id,
            "source_edge_registry target_resource",
        )?;
        require_equal(
            &edge.mainline_call_doc,
            &binding.mainline_call_doc,
            &edge.edge_id,
            "source_edge_registry mainline_call_doc",
        )?;
        let mainline_path = root.join(&edge.mainline_call_doc);
        let mainline = load_mainline_doc(&mainline_path)?;
        let row = mainline
            .call_table
            .iter()
            .find(|row| {
                row.step == edge.call_table_step
                    && row.resource_operation.as_deref() == Some(edge.operation_id.as_str())
            })
            .ok_or_else(|| {
                format!(
                    "source_edge_registry `{}` has no matching call_table row `{}` for operation `{}` in `{}`",
                    edge.edge_id, edge.call_table_step, edge.operation_id, edge.mainline_call_doc
                )
            })?;
        if row.source_resource.as_deref() != Some(edge.source_resource.as_str())
            || row.target_resource.as_deref() != Some(edge.target_resource.as_str())
            || row.file_path != edge.file_path
            || row.symbol_path != edge.symbol_path
        {
            return Err(format!(
                "source_edge_registry `{}` does not match call_table row `{}` in `{}`",
                edge.edge_id, edge.call_table_step, edge.mainline_call_doc
            ));
        }
        let key = source_edge_key(
            &edge.mainline_call_doc,
            &edge.call_table_step,
            &edge.operation_id,
            &edge.source_resource,
            &edge.target_resource,
            &edge.file_path,
            &edge.symbol_path,
        );
        if !source_edge_registry.insert(key) {
            return Err(format!(
                "duplicate source_edge_registry binding for `{}` step `{}` operation `{}`",
                edge.mainline_call_doc, edge.call_table_step, edge.operation_id
            ));
        }
    }

    for source_path in mainline_source_paths(root)? {
        let doc = load_mainline_doc(&source_path)?;
        for row in &doc.call_table {
            if let Some(resource_operation) = &row.resource_operation {
                let (expected_source, expected_target) =
                    operation_lookup.get(resource_operation).ok_or_else(|| {
                        format!(
                            "mainline `{}` step `{}` references unknown resource_operation `{}`",
                            doc.feature_id, row.step, resource_operation
                        )
                    })?;
                let source_resource = row.source_resource.as_deref().ok_or_else(|| {
                    format!(
                        "mainline `{}` step `{}` resource_operation `{}` must declare source_resource",
                        doc.feature_id, row.step, resource_operation
                    )
                })?;
                let target_resource = row.target_resource.as_deref().ok_or_else(|| {
                    format!(
                        "mainline `{}` step `{}` resource_operation `{}` must declare target_resource",
                        doc.feature_id, row.step, resource_operation
                    )
                })?;
                if source_resource != *expected_source || target_resource != *expected_target {
                    return Err(format!(
                        "mainline `{}` step `{}` resource_operation `{}` endpoints mismatch: expected `{}` -> `{}`, got `{}` -> `{}`",
                        doc.feature_id,
                        row.step,
                        resource_operation,
                        expected_source,
                        expected_target,
                        source_resource,
                        target_resource
                    ));
                }
                let key = source_edge_key(
                    &doc.mainline_call_doc,
                    &row.step,
                    resource_operation,
                    source_resource,
                    target_resource,
                    &row.file_path,
                    &row.symbol_path,
                );
                if !source_edge_registry.contains(&key) {
                    return Err(format!(
                        "mainline `{}` step `{}` resource_operation `{}` is missing from source_edge_registry",
                        doc.feature_id, row.step, resource_operation
                    ));
                }
            } else if row.source_resource.is_some() || row.target_resource.is_some() {
                return Err(format!(
                    "mainline `{}` step `{}` declares source/target resource without resource_operation",
                    doc.feature_id, row.step
                ));
            }
        }
    }

    let mut relation_rule_ids = BTreeSet::new();
    let mut relation_rule_pairs = BTreeSet::new();
    let mut direct_relation_pairs = BTreeSet::new();
    let mut indirect_relation_rules: BTreeMap<(String, String), Vec<String>> = BTreeMap::new();
    for rule in &map.relation_rules {
        require_non_empty(&rule.rule_id, "relation_rules.rule_id")?;
        if !relation_rule_ids.insert(rule.rule_id.clone()) {
            return Err(format!(
                "duplicate resource relation rule_id `{}` in docs/resource-maps/core.json",
                rule.rule_id
            ));
        }
        require_non_empty(&rule.reason, "relation_rules.reason")?;
        require_known_resource(&resources, &rule.source_resource, &rule.rule_id)?;
        require_known_resource(&resources, &rule.target_resource, &rule.rule_id)?;
        if !relation_rule_pairs.insert((rule.source_resource.clone(), rule.target_resource.clone()))
        {
            return Err(format!(
                "duplicate resource relation rule pair `{}` -> `{}` in docs/resource-maps/core.json",
                rule.source_resource, rule.target_resource
            ));
        }
        if rule.allowed_direct {
            direct_relation_pairs
                .insert((rule.source_resource.clone(), rule.target_resource.clone()));
        } else {
            indirect_relation_rules.insert(
                (rule.source_resource.clone(), rule.target_resource.clone()),
                rule.via_resources.clone(),
            );
        }
        if rule.allowed_direct && !rule.via_resources.is_empty() {
            return Err(format!(
                "direct resource relation `{}` must not declare via_resources",
                rule.rule_id
            ));
        }
        if !rule.allowed_direct && rule.via_resources.is_empty() {
            return Err(format!(
                "indirect resource relation `{}` must declare via_resources",
                rule.rule_id
            ));
        }
        if !rule.allowed_direct
            && operation_pairs
                .contains(&(rule.source_resource.clone(), rule.target_resource.clone()))
        {
            return Err(format!(
                "resource relation `{}` forbids direct `{}` -> `{}` but an operation binding declares that direct pair",
                rule.rule_id, rule.source_resource, rule.target_resource
            ));
        }
        for via in &rule.via_resources {
            require_known_resource(&resources, via, &rule.rule_id)?;
        }
    }
    for (source_resource, target_resource) in &bound_operation_pairs {
        if !direct_relation_pairs.contains(&(source_resource.clone(), target_resource.clone())) {
            return Err(format!(
                "bound resource operation pair `{source_resource}` -> `{target_resource}` must have an allowed_direct relation rule"
            ));
        }
    }

    let mut source_shortcut_gate_pairs = BTreeSet::new();
    for gate in &map.source_shortcut_gates {
        if !source_shortcut_gate_pairs
            .insert((gate.source_resource.clone(), gate.target_resource.clone()))
        {
            return Err(format!(
                "duplicate source_shortcut_gates pair `{}` -> `{}`",
                gate.source_resource, gate.target_resource
            ));
        }
    }
    let mut precise_source_edge_gate_pairs = BTreeSet::new();
    for gate in &map.precise_source_edge_gates {
        if !precise_source_edge_gate_pairs
            .insert((gate.source_resource.clone(), gate.target_resource.clone()))
        {
            return Err(format!(
                "duplicate precise_source_edge_gates pair `{}` -> `{}`",
                gate.source_resource, gate.target_resource
            ));
        }
    }
    let mut forbidden_direct_pairs = BTreeSet::new();

    for relation in &map.forbidden_direct_relations {
        let context = format!(
            "forbidden direct relation {} -> {}",
            relation.source_resource, relation.target_resource
        );
        require_known_resource(&resources, &relation.source_resource, &context)?;
        require_known_resource(&resources, &relation.target_resource, &context)?;
        if !forbidden_direct_pairs.insert((
            relation.source_resource.clone(),
            relation.target_resource.clone(),
        )) {
            return Err(format!("{context} is duplicated"));
        }
        require_non_empty(&relation.reason, "forbidden_direct_relations.reason")?;
        if relation.required_via.is_empty() {
            return Err(format!("{context} must declare required_via"));
        }
        if direct_relation_pairs.contains(&(
            relation.source_resource.clone(),
            relation.target_resource.clone(),
        )) {
            return Err(format!(
                "{context} conflicts with an allowed_direct relation rule"
            ));
        }
        if operation_pairs.contains(&(
            relation.source_resource.clone(),
            relation.target_resource.clone(),
        )) {
            return Err(format!(
                "{context} is forbidden but an operation binding declares that direct pair"
            ));
        }
        for via in &relation.required_via {
            require_known_resource(&resources, via, &context)?;
        }
        let indirect_via = indirect_relation_rules
            .get(&(
                relation.source_resource.clone(),
                relation.target_resource.clone(),
            ))
            .ok_or_else(|| format!("{context} must have a matching indirect relation rule"))?;
        if indirect_via != &relation.required_via {
            return Err(format!(
                "{context} required_via must match the indirect relation rule via_resources"
            ));
        }
        require_non_empty(&relation.source_gate_status, "source_gate_status")?;
        require_non_empty(&relation.source_gate_reason, "source_gate_reason")?;
        match parse_source_gate_status(&relation.source_gate_status, &context)? {
            SourceGateStatus::Checked => {
                if !source_shortcut_gate_pairs.contains(&(
                    relation.source_resource.clone(),
                    relation.target_resource.clone(),
                )) {
                    return Err(format!(
                        "{context} has source_gate_status=checked but no matching source_shortcut_gates entry"
                    ));
                }
            }
            SourceGateStatus::PreciseChecked => {
                if !precise_source_edge_gate_pairs.contains(&(
                    relation.source_resource.clone(),
                    relation.target_resource.clone(),
                )) {
                    return Err(format!(
                        "{context} has source_gate_status=precise_checked but no matching precise_source_edge_gates entry"
                    ));
                }
            }
        }
    }

    let resource_owner_crates: BTreeMap<&str, &str> = map
        .resources
        .iter()
        .map(|resource| {
            (
                resource.resource_type.as_str(),
                resource.owner_crate.as_str(),
            )
        })
        .collect();

    for gate in &map.source_shortcut_gates {
        require_known_resource(&resources, &gate.source_resource, "source_shortcut_gates")?;
        require_known_resource(&resources, &gate.target_resource, "source_shortcut_gates")?;
        require_non_empty(&gate.reason, "source_shortcut_gates.reason")?;
        if gate.forbidden_packages.is_empty() && gate.forbidden_import_tokens.is_empty() {
            return Err(format!(
                "source shortcut gate `{}` -> `{}` must declare forbidden_packages or forbidden_import_tokens",
                gate.source_resource, gate.target_resource
            ));
        }
        for package in &gate.forbidden_packages {
            require_non_empty(package, "source_shortcut_gates.forbidden_packages")?;
        }
        for token in &gate.forbidden_import_tokens {
            require_non_empty(token, "source_shortcut_gates.forbidden_import_tokens")?;
        }
        if !forbidden_direct_pairs
            .contains(&(gate.source_resource.clone(), gate.target_resource.clone()))
        {
            return Err(format!(
                "source shortcut gate `{}` -> `{}` must reference a forbidden_direct_relations pair",
                gate.source_resource, gate.target_resource
            ));
        }
        let source_crate = resource_owner_crates
            .get(gate.source_resource.as_str())
            .ok_or_else(|| {
                format!(
                    "source shortcut gate references resource without owner crate `{}`",
                    gate.source_resource
                )
            })?;
        verify_source_shortcut_gate(root, source_crate, gate)?;
    }

    for gate in &map.precise_source_edge_gates {
        require_known_resource(
            &resources,
            &gate.source_resource,
            "precise_source_edge_gates",
        )?;
        require_known_resource(
            &resources,
            &gate.target_resource,
            "precise_source_edge_gates",
        )?;
        if !forbidden_direct_pairs
            .contains(&(gate.source_resource.clone(), gate.target_resource.clone()))
        {
            return Err(format!(
                "precise source edge gate `{}` -> `{}` must reference a forbidden_direct_relations pair",
                gate.source_resource, gate.target_resource
            ));
        }
        verify_precise_source_edge_gate(root, gate)?;
    }

    Ok(())
}

fn verify_required_core_resources(resources: &BTreeSet<String>) -> Result<(), String> {
    const REQUIRED_CORE_RESOURCES: &[&str] = &[
        "config",
        "session",
        "turn",
        "request_context",
        "provider_request",
        "provider_response",
        "tool_call",
        "workspace_path",
        "task",
        "agent",
        "timer",
        "error",
        "metadata",
        "debug_trace",
        "ui_projection",
        "runtime_command",
        "checkpoint",
        "node_pairing",
        "instruction_capability",
    ];

    for resource in REQUIRED_CORE_RESOURCES {
        if !resources.contains(*resource) {
            return Err(format!(
                "core resource map missing required resource `{resource}`"
            ));
        }
    }
    Ok(())
}

fn source_edge_key(
    mainline_call_doc: &str,
    call_table_step: &str,
    operation_id: &str,
    source_resource: &str,
    target_resource: &str,
    file_path: &str,
    symbol_path: &str,
) -> String {
    format!(
        "{mainline_call_doc}\n{call_table_step}\n{operation_id}\n{source_resource}\n{target_resource}\n{file_path}\n{symbol_path}"
    )
}

fn parse_feature_map_resource_ownership(
    feature_map: &str,
) -> Result<BTreeMap<String, BTreeSet<String>>, String> {
    let section = feature_map
        .split("## Resource Ownership Index")
        .nth(1)
        .and_then(|tail| tail.split("\n## ").next())
        .ok_or_else(|| "feature map missing `## Resource Ownership Index` section".to_owned())?;
    let mut ownership = BTreeMap::new();
    for line in section.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('|')
            || trimmed.contains("| ---")
            || trimmed.contains("| feature_id ")
        {
            continue;
        }
        let cells: Vec<&str> = trimmed
            .trim_matches('|')
            .split('|')
            .map(str::trim)
            .collect();
        if cells.len() < 3 {
            continue;
        }
        let feature_id = trim_backticks(cells[0]);
        if feature_id.is_empty() {
            continue;
        }
        let resource_map_doc = trim_backticks(cells[2]);
        if resource_map_doc != "docs/resource-maps/core.json" {
            return Err(format!(
                "feature map Resource Ownership Index `{feature_id}` must reference `docs/resource-maps/core.json`, got `{resource_map_doc}`"
            ));
        }
        let mut resources = BTreeSet::new();
        for resource in cells[1].split(',') {
            let resource = trim_backticks(resource.trim());
            if resource.is_empty() {
                continue;
            }
            resources.insert(resource.to_owned());
        }
        if resources.is_empty() {
            return Err(format!(
                "feature map Resource Ownership Index `{feature_id}` must list at least one resource"
            ));
        }
        if ownership.insert(feature_id.to_owned(), resources).is_some() {
            return Err(format!(
                "feature map Resource Ownership Index duplicates feature `{feature_id}`"
            ));
        }
    }
    if ownership.is_empty() {
        return Err("feature map Resource Ownership Index has no resource rows".to_owned());
    }
    Ok(ownership)
}

fn parse_feature_map_seed_owners(feature_map: &str) -> Result<BTreeMap<String, String>, String> {
    let mut owners = BTreeMap::new();
    let mut current_feature: Option<String> = None;
    let mut current_owner: Option<String> = None;

    for line in feature_map.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("## ") && current_feature.is_some() {
            if let Some(feature_id) = current_feature.take() {
                let owner = current_owner.take().ok_or_else(|| {
                    format!("feature map seed entry `{feature_id}` must declare `owner`")
                })?;
                if owners.insert(feature_id.clone(), owner).is_some() {
                    return Err(format!(
                        "feature map seed entry `{feature_id}` is duplicated"
                    ));
                }
            }
            continue;
        }
        if trimmed.starts_with("### `") && trimmed.ends_with('`') {
            if let Some(feature_id) = current_feature.take() {
                let owner = current_owner.take().ok_or_else(|| {
                    format!("feature map seed entry `{feature_id}` must declare `owner`")
                })?;
                if owners.insert(feature_id.clone(), owner).is_some() {
                    return Err(format!(
                        "feature map seed entry `{feature_id}` is duplicated"
                    ));
                }
            }
            current_feature = Some(trimmed[5..trimmed.len() - 1].to_owned());
            current_owner = None;
            continue;
        }
        if current_feature.is_some() && current_owner.is_none() && trimmed.starts_with("- owner:") {
            let owner = trimmed.trim_start_matches("- owner:").trim();
            if owner.is_empty() {
                return Err("feature map seed owner must not be empty".to_owned());
            }
            current_owner = Some(owner.to_owned());
        }
    }

    if let Some(feature_id) = current_feature.take() {
        let owner = current_owner
            .take()
            .ok_or_else(|| format!("feature map seed entry `{feature_id}` must declare `owner`"))?;
        if owners.insert(feature_id.clone(), owner).is_some() {
            return Err(format!(
                "feature map seed entry `{feature_id}` is duplicated"
            ));
        }
    }

    if owners.is_empty() {
        return Err("feature map has no seed owner entries".to_owned());
    }
    Ok(owners)
}

fn trim_backticks(value: &str) -> &str {
    value.trim().trim_matches('`').trim()
}

fn verify_source_shortcut_gate(
    root: &Path,
    source_crate: &str,
    gate: &ResourceMapSourceShortcutGate,
) -> Result<(), String> {
    let cargo_path = root.join(source_crate).join("Cargo.toml");
    let cargo = fs::read_to_string(&cargo_path).map_err(|err| {
        format!(
            "read source shortcut Cargo.toml {}: {err}",
            cargo_path.display()
        )
    })?;
    for package in &gate.forbidden_packages {
        if cargo.contains(package) {
            return Err(format!(
                "resource shortcut gate `{}` -> `{}` forbids dependency `{}` in {}",
                gate.source_resource,
                gate.target_resource,
                package,
                cargo_path.display()
            ));
        }
    }

    for file_path in rust_source_paths_under(&root.join(source_crate))? {
        let source = fs::read_to_string(&file_path)
            .map_err(|err| format!("read source shortcut file {}: {err}", file_path.display()))?;
        for token in &gate.forbidden_import_tokens {
            if source.contains(token) {
                return Err(format!(
                    "resource shortcut gate `{}` -> `{}` forbids import/reference token `{}` in {}",
                    gate.source_resource,
                    gate.target_resource,
                    token,
                    file_path.display()
                ));
            }
        }
    }

    Ok(())
}

#[derive(Debug)]
enum SourceGateStatus {
    Checked,
    PreciseChecked,
}

fn parse_source_gate_status(status: &str, context: &str) -> Result<SourceGateStatus, String> {
    match status {
        "checked" => Ok(SourceGateStatus::Checked),
        "precise_checked" => Ok(SourceGateStatus::PreciseChecked),
        other => Err(format!(
            "{context} has unsupported source_gate_status `{other}`"
        )),
    }
}

fn rust_source_paths_under(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    collect_rust_source_paths_under(dir, &mut paths)?;
    Ok(paths)
}

fn collect_rust_source_paths_under(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|err| format!("read source directory {}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read source directory entry: {err}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("read file type {}: {err}", path.display()))?;
        if file_type.is_dir() {
            if entry.file_name() == "target" {
                continue;
            }
            collect_rust_source_paths_under(&path, paths)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
    Ok(())
}

fn verify_precise_source_edge_gate(
    root: &Path,
    gate: &ResourceMapPreciseSourceEdgeGate,
) -> Result<(), String> {
    require_non_empty(&gate.file_path, "precise_source_edge_gates.file_path")?;
    require_non_empty(&gate.symbol_path, "precise_source_edge_gates.symbol_path")?;
    require_non_empty(&gate.reason, "precise_source_edge_gates.reason")?;
    if gate.required_tokens.is_empty() {
        return Err(format!(
            "precise source edge gate `{}` -> `{}` must declare required_tokens",
            gate.source_resource, gate.target_resource
        ));
    }
    let file_path = root.join(&gate.file_path);
    let source = fs::read_to_string(&file_path).map_err(|err| {
        format!(
            "read precise source edge file {}: {err}",
            file_path.display()
        )
    })?;
    if !symbol_resolves_in_files(
        root,
        std::slice::from_ref(&gate.file_path),
        &gate.symbol_path,
    )? {
        return Err(format!(
            "precise source edge gate `{}` -> `{}` references missing symbol `{}` in `{}`",
            gate.source_resource, gate.target_resource, gate.symbol_path, gate.file_path
        ));
    }
    let body = extract_function_body(&source, &gate.symbol_path).ok_or_else(|| {
        format!(
            "precise source edge gate `{}` -> `{}` could not extract function body for `{}` in `{}`",
            gate.source_resource, gate.target_resource, gate.symbol_path, gate.file_path
        )
    })?;
    for token in &gate.required_tokens {
        require_non_empty(token, "precise_source_edge_gates.required_tokens")?;
        if !body.contains(token) {
            return Err(format!(
                "precise source edge gate `{}` -> `{}` requires token `{}` in `{}` body",
                gate.source_resource, gate.target_resource, token, gate.symbol_path
            ));
        }
    }
    for token in &gate.forbidden_tokens {
        require_non_empty(token, "precise_source_edge_gates.forbidden_tokens")?;
        if body.contains(token) {
            return Err(format!(
                "precise source edge gate `{}` -> `{}` forbids token `{}` in `{}` body",
                gate.source_resource, gate.target_resource, token, gate.symbol_path
            ));
        }
    }
    Ok(())
}

fn extract_function_body<'a>(source: &'a str, symbol_path: &str) -> Option<&'a str> {
    let symbol_tail = symbol_path
        .rsplit("::")
        .next()
        .unwrap_or(symbol_path)
        .trim();
    let pattern = format!("fn {symbol_tail}(");
    let function_start = source.find(&pattern)?;
    let signature_tail = &source[function_start..];
    let body_start_offset = signature_tail.find('{')?;
    let body_start = function_start + body_start_offset;
    let mut depth = 0_i32;
    for (offset, ch) in source[body_start..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let end = body_start + offset + ch.len_utf8();
                    return source.get(body_start..end);
                }
            }
            _ => {}
        }
    }
    None
}

fn require_non_empty(value: &str, field: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("resource map field `{field}` must not be empty"));
    }
    Ok(())
}

fn require_known_resource(
    resources: &BTreeSet<String>,
    resource_type: &str,
    context: &str,
) -> Result<(), String> {
    if !resources.contains(resource_type) {
        return Err(format!(
            "resource map `{context}` references unknown resource `{resource_type}`"
        ));
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
    let mut generated = Vec::new();
    let mut wiki_index = String::new();
    wiki_index.push_str("# Generated Wiki Index\n\n");
    wiki_index.push_str(
        "Generated wiki artifacts from machine-readable mainline call source files. Do not edit by hand.\n\n",
    );
    for source_path in mainline_source_paths(root)? {
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

fn verify_mainline_manifest_links(root: &Path) -> Result<(), String> {
    let feature_map_path = root.join("docs/architecture/feature-map.md");
    let feature_map = fs::read_to_string(&feature_map_path)
        .map_err(|err| format!("read feature map {}: {err}", feature_map_path.display()))?;

    for source_path in mainline_source_paths(root)? {
        let doc = load_mainline_doc(&source_path)?;
        let source_rel = relative_slash_path(root, &source_path)?;
        let expected_mainline = format!("docs/mainline-calls/{}.json", doc.feature_id);
        let expected_function_map = format!("docs/function-maps/{}.md", doc.feature_id);
        let expected_test_design = format!("docs/testing/{}.md", doc.feature_id);
        let expected_wiki = format!("docs/wiki/{}.md", doc.feature_id);

        require_equal(
            &source_rel,
            &expected_mainline,
            &doc.feature_id,
            "source path",
        )?;
        require_equal(
            &doc.mainline_call_doc,
            &expected_mainline,
            &doc.feature_id,
            "mainline_call_doc",
        )?;
        require_equal(
            &doc.function_map_doc,
            &expected_function_map,
            &doc.feature_id,
            "function_map_doc",
        )?;
        require_equal(
            &doc.test_design_doc,
            &expected_test_design,
            &doc.feature_id,
            "test_design_doc",
        )?;
        require_equal(
            &doc.generated_wiki_doc,
            &expected_wiki,
            &doc.feature_id,
            "generated_wiki_doc",
        )?;

        let function_map_path = root.join(&doc.function_map_doc);
        let function_map = fs::read_to_string(&function_map_path)
            .map_err(|err| format!("read function map {}: {err}", function_map_path.display()))?;
        let test_design_path = root.join(&doc.test_design_doc);
        let test_design = fs::read_to_string(&test_design_path)
            .map_err(|err| format!("read test design {}: {err}", test_design_path.display()))?;
        let generated_wiki_path = root.join(&doc.generated_wiki_doc);
        if !generated_wiki_path.is_file() {
            return Err(format!(
                "mainline manifest `{}` references missing generated wiki `{}`",
                doc.feature_id,
                generated_wiki_path.display()
            ));
        }

        require_contains(
            &function_map,
            &format!("- feature_id: `{}`", doc.feature_id),
            &doc.function_map_doc,
        )?;
        require_contains(&function_map, &doc.mainline_call_doc, &doc.function_map_doc)?;
        require_contains(
            &test_design,
            &format!("- feature_id: `{}`", doc.feature_id),
            &doc.test_design_doc,
        )?;
        require_contains(
            &feature_map,
            &doc.mainline_call_doc,
            "docs/architecture/feature-map.md",
        )?;
        require_contains(
            &feature_map,
            &doc.generated_wiki_doc,
            "docs/architecture/feature-map.md",
        )?;
        for row in &doc.call_table {
            if let Some(resource_operation) = &row.resource_operation
                && !doc.resource_operations.contains(resource_operation)
            {
                return Err(format!(
                    "mainline `{}` step `{}` references resource_operation `{}` that is not listed in resource_operations",
                    doc.feature_id, row.step, resource_operation
                ));
            }
        }
    }

    Ok(())
}

fn verify_mainline_call_table_bindings(root: &Path) -> Result<(), String> {
    for source_path in mainline_source_paths(root)? {
        let doc = load_mainline_doc(&source_path)?;
        for row in &doc.call_table {
            match row.binding_status.as_str() {
                "pending" => continue,
                "bound" => {}
                status => {
                    return Err(format!(
                        "mainline `{}` step `{}` has unsupported binding_status `{}`",
                        doc.feature_id, row.step, status
                    ));
                }
            }

            let file_paths = split_binding_segments(&row.file_path);
            let symbol_paths = split_binding_segments(&row.symbol_path);
            if file_paths.is_empty() {
                return Err(format!(
                    "mainline `{}` step `{}` has no file_path binding",
                    doc.feature_id, row.step
                ));
            }
            if symbol_paths.is_empty() {
                return Err(format!(
                    "mainline `{}` step `{}` has no symbol_path binding",
                    doc.feature_id, row.step
                ));
            }

            for file_path in &file_paths {
                let full_path = root.join(file_path);
                if !full_path.is_file() {
                    return Err(format!(
                        "mainline `{}` step `{}` references missing file `{}`",
                        doc.feature_id, row.step, file_path
                    ));
                }
            }

            for symbol_path in &symbol_paths {
                if !symbol_resolves_in_files(root, &file_paths, symbol_path)? {
                    return Err(format!(
                        "mainline `{}` step `{}` references missing symbol `{}` in `{}`",
                        doc.feature_id, row.step, symbol_path, row.file_path
                    ));
                }
            }
        }
    }

    Ok(())
}

fn verify_ci_cd_gate_commands(root: &Path) -> Result<(), String> {
    let makefile =
        fs::read_to_string(root.join("Makefile")).map_err(|err| format!("read Makefile: {err}"))?;
    require_contains(
        &makefile,
        ".PHONY: build fmt clippy test mainlines gates ci verify-webui-online verify-webui-release-online release install-global install-symlink install-launchd install-launchdS install-worker-launchd install-worker-launchdS restart-launchd restart-launchdS restart-worker-launchd restart-worker-launchdS uninstall-launchd uninstall-launchdS uninstall-worker-launchd uninstall-worker-launchdS launchd-status launchd-statusS worker-launchd-status worker-launchd-statusS launchd-logs launchd-logsS worker-launchd-logs worker-launchd-logsS hooks",
        "Makefile",
    )?;
    require_contains(
        &makefile,
        "mainlines:\n\tcargo run -p xtask -- mainlines check",
        "Makefile",
    )?;
    require_contains(
        &makefile,
        "ci: build fmt clippy test mainlines gates",
        "Makefile",
    )?;
    require_contains(&makefile, "release:\n\tscripts/release.sh", "Makefile")?;
    require_contains(
        &makefile,
        "verify-webui-online:\n\tscripts/verify-webui-online.sh",
        "Makefile",
    )?;
    require_contains(
        &makefile,
        "verify-webui-release-online:\n\tscripts/verify-webui-release-online.sh",
        "Makefile",
    )?;
    let webui_online = fs::read_to_string(root.join("scripts/verify-webui-online.sh"))
        .map_err(|err| format!("read scripts/verify-webui-online.sh: {err}"))?;
    require_contains(
        &webui_online,
        "FREEHAND_WEBUI_BASE_URL:-http://127.0.0.1:4042/",
        "scripts/verify-webui-online.sh",
    )?;
    require_contains(
        &webui_online,
        "FREEHAND_WEBUI_HEALTH_URL:-http://127.0.0.1:4042/health",
        "scripts/verify-webui-online.sh",
    )?;
    require_contains(
        &webui_online,
        "FREEHAND_WEBUI_ADP_URL:-ws://127.0.0.1:4042/adp",
        "scripts/verify-webui-online.sh",
    )?;
    require_contains(
        &webui_online,
        "FREEHAND_WEBUI_CLI:-$HOME/.local/bin/freehand-cliS",
        "scripts/verify-webui-online.sh",
    )?;
    require_contains(
        &webui_online,
        "FREEHAND_WEBUI_PROFILE:-4042",
        "scripts/verify-webui-online.sh",
    )?;
    let webui_release = fs::read_to_string(root.join("scripts/verify-webui-release-online.sh"))
        .map_err(|err| format!("read scripts/verify-webui-release-online.sh: {err}"))?;
    require_contains(
        &webui_release,
        "FREEHAND_WEBUI_BASE_URL:-http://127.0.0.1:4041/",
        "scripts/verify-webui-release-online.sh",
    )?;
    require_contains(
        &webui_release,
        "FREEHAND_WEBUI_HEALTH_URL:-http://127.0.0.1:4041/health",
        "scripts/verify-webui-release-online.sh",
    )?;
    require_contains(
        &webui_release,
        "FREEHAND_WEBUI_ADP_URL:-ws://127.0.0.1:4041/adp",
        "scripts/verify-webui-release-online.sh",
    )?;
    require_contains(
        &webui_release,
        "FREEHAND_WEBUI_CLI:-$HOME/.local/bin/freehand-cli}",
        "scripts/verify-webui-release-online.sh",
    )?;
    require_contains(
        &webui_release,
        "FREEHAND_WEBUI_PROFILE:-4041",
        "scripts/verify-webui-release-online.sh",
    )?;
    require_contains(
        &makefile,
        "install-global:\n\tscripts/install-global.sh",
        "Makefile",
    )?;
    require_contains(
        &makefile,
        "install-symlink:\n\tscripts/install-symlink.sh",
        "Makefile",
    )?;
    require_contains(
        &makefile,
        "install-launchd:\n\tscripts/install-launchd.sh",
        "Makefile",
    )?;
    require_contains(
        &makefile,
        "install-launchdS:\n\tscripts/install-launchd.sh installS",
        "Makefile",
    )?;
    require_contains(
        &makefile,
        "install-worker-launchdS:\n\tscripts/install-launchd.sh installWorkerS",
        "Makefile",
    )?;
    require_contains(
        &makefile,
        "restart-launchd:\n\tscripts/install-launchd.sh restart",
        "Makefile",
    )?;
    require_contains(
        &makefile,
        "restart-launchdS:\n\tscripts/install-launchd.sh restartS",
        "Makefile",
    )?;
    require_contains(
        &makefile,
        "restart-worker-launchdS:\n\tscripts/install-launchd.sh restartWorkerS",
        "Makefile",
    )?;
    require_contains(
        &makefile,
        "uninstall-launchd:\n\tscripts/uninstall-launchd.sh",
        "Makefile",
    )?;
    require_contains(
        &makefile,
        "uninstall-launchdS:\n\tscripts/uninstall-launchd.sh uninstallS",
        "Makefile",
    )?;
    require_contains(
        &makefile,
        "uninstall-worker-launchdS:\n\tscripts/uninstall-launchd.sh uninstallWorkerS",
        "Makefile",
    )?;
    let install_launchd = fs::read_to_string(root.join("scripts/install-launchd.sh"))
        .map_err(|err| format!("read scripts/install-launchd.sh: {err}"))?;
    require_contains(
        &install_launchd,
        "if [[ \"$profile_suffix\" == \"S\" ]]; then",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "printf '127.0.0.1:%s\\n' \"$port\"",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "elif [[ -f \"$env_file\" ]]; then",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "env_bind=\"$(awk -F= '$1 == \"FREEHAND_DAEMON_BIND\"",
        "scripts/install-launchd.sh",
    )?;
    if install_launchd.contains("workdir=\"${FREEHAND_DAEMON_WORKDIR:-\"$repo_root\"}\"") {
        return Err(
            "scripts/install-launchd.sh must not default the master daemon workdir to the repository root"
                .to_owned(),
        );
    }
    require_contains(
        &install_launchd,
        "workdir=\"${FREEHAND_DAEMON_WORKDIR:-\"$runtime_home\"}\"",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "mkdir -p \"$runtime_home\" \"$logs_dir\" \"$workdir\"",
        "scripts/install-launchd.sh",
    )?;
    if !install_launchd.contains("set -a; [ -f \"$env_file\" ] && . \"$env_file\"; set +a;")
        && !install_launchd
            .contains("set -a; [ -f \"$env_file\" ] &amp;&amp; . \"$env_file\"; set +a;")
    {
        return Err(
            "mainline manifest cross-link missing launchd env-file sourcing in scripts/install-launchd.sh"
                .to_string(),
        );
    }
    require_contains(&install_launchd, "restartS)", "scripts/install-launchd.sh")?;
    require_contains(
        &install_launchd,
        "installWorkerS|restartWorkerS)",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "default_label=\"com.freehand.workerS\"",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "exec \"$daemon_bin\" serve --agent \"$agent\"</string>",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "worker requires FREEHAND_PAIR_TOKEN_SHARED",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "copy_worker_provider_env_from_master",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "_KEY|CREDENTIAL|SECRET",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "wait_for_worker_service",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "kill -0 \"$service_pid\"",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "write_launchd_plist",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "restart_launchd",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "launchctl bootout \"gui/$(id -u)\" \"$plist_path\"",
        "scripts/install-launchd.sh",
    )?;
    require_contains(
        &install_launchd,
        "launchctl bootstrap \"gui/$(id -u)\" \"$plist_path\"",
        "scripts/install-launchd.sh",
    )?;

    let pre_push = fs::read_to_string(root.join(".githooks/pre-push"))
        .map_err(|err| format!("read .githooks/pre-push: {err}"))?;
    require_contains(&pre_push, "make ci", ".githooks/pre-push")?;

    let ci_workflow = fs::read_to_string(root.join(".github/workflows/ci.yml"))
        .map_err(|err| format!("read .github/workflows/ci.yml: {err}"))?;
    require_contains(&ci_workflow, "run: make ci", ".github/workflows/ci.yml")?;

    let release_workflow = fs::read_to_string(root.join(".github/workflows/release.yml"))
        .map_err(|err| format!("read .github/workflows/release.yml: {err}"))?;
    require_contains(
        &release_workflow,
        "run: make ci",
        ".github/workflows/release.yml",
    )?;
    require_contains(
        &release_workflow,
        "run: scripts/release.sh",
        ".github/workflows/release.yml",
    )?;

    Ok(())
}

fn verify_source_search_policy(root: &Path) -> Result<(), String> {
    let ignore =
        fs::read_to_string(root.join(".ignore")).map_err(|err| format!("read .ignore: {err}"))?;
    for snippet in [
        "target/",
        "dist/",
        "artifacts/",
        "docs/wiki/",
        ".mempalace/",
        "memory/*-mempalace-corpus/",
        "test-palaces/",
        "**/build/",
        "**/.gradle/",
        "**/node_modules/",
    ] {
        require_contains(&ignore, snippet, ".ignore")?;
    }

    let script = fs::read_to_string(root.join("scripts/source-search.sh"))
        .map_err(|err| format!("read scripts/source-search.sh: {err}"))?;
    for snippet in [
        "exec rg --hidden",
        "--glob=!artifacts/**",
        "--glob=!target/**",
        "--glob=!dist/**",
        "--glob=!docs/wiki/**",
        "--glob=!.mempalace/**",
        "--glob=!memory/*-mempalace-corpus/**",
        "--glob=!test-palaces/**",
        "for arg in \"$@\"; do",
        "--no-ignore",
        "--unrestricted",
        "exec rg --hidden \"$@\" \"${exclude_globs[@]}\" \"${search_roots[@]}\"",
        "docs/architecture",
        "docs/function-maps",
        "docs/mainline-calls",
        "docs/testing",
        "crates",
        "apps",
        "xtask",
    ] {
        require_contains(&script, snippet, "scripts/source-search.sh")?;
    }
    for forbidden in [
        "CACHE.md",
        "MEMORY.md",
        "note.md",
        "artifacts",
        "target",
        "dist",
        "docs/wiki",
        ".mempalace",
        "memory",
        "test-palaces",
        "tmp",
    ] {
        if script.contains(&format!("\"{forbidden}\"")) {
            return Err(format!(
                "scripts/source-search.sh must not include `{forbidden}` as an implementation-search root"
            ));
        }
    }

    let skill = fs::read_to_string(root.join(".agents/skills/freehand-dev/SKILL.md"))
        .map_err(|err| format!("read .agents/skills/freehand-dev/SKILL.md: {err}"))?;
    for snippet in [
        "Debug/search truth is source-first",
        "Do not search generated or runtime output when locating implementation truth",
        "Generated artifacts may be opened only as verification evidence",
        "scripts/source-search.sh",
    ] {
        require_contains(&skill, snippet, ".agents/skills/freehand-dev/SKILL.md")?;
    }

    let debug_workflow =
        fs::read_to_string(root.join("docs/architecture/dev-debug-workflow.md"))
            .map_err(|err| format!("read docs/architecture/dev-debug-workflow.md: {err}"))?;
    for snippet in [
        "Source-Only Search Rule",
        "scripts/source-search.sh",
        "not as implementation search roots",
    ] {
        require_contains(
            &debug_workflow,
            snippet,
            "docs/architecture/dev-debug-workflow.md",
        )?;
    }

    let dev_gates = fs::read_to_string(root.join("docs/architecture/dev-gates.md"))
        .map_err(|err| format!("read docs/architecture/dev-gates.md: {err}"))?;
    for snippet in [
        "Source Search Boundary Gate",
        "`xtask gates check` validates source-only search policy",
        "generated outputs remain excluded from default implementation search",
    ] {
        require_contains(&dev_gates, snippet, "docs/architecture/dev-gates.md")?;
    }

    Ok(())
}

fn verify_data_control_boundaries(root: &Path) -> Result<(), String> {
    let contracts_path = root.join("crates/freehand-contracts/src/lib.rs");
    let contracts = fs::read_to_string(&contracts_path)
        .map_err(|err| format!("read contracts source {}: {err}", contracts_path.display()))?;
    for block in extract_struct_blocks(&contracts) {
        if !block.name.starts_with("ReasonReq") {
            continue;
        }
        for field in parse_struct_fields(&block.body) {
            if is_forbidden_request_field_type(field.ty) {
                return Err(format!(
                    "request-node `{}` introduces forbidden control/metadata/debug type `{}` via field `{}`",
                    block.name, field.ty, field.name
                ));
            }
            if is_forbidden_request_field_name(field.name) {
                return Err(format!(
                    "request-node `{}` introduces forbidden control/metadata/debug field `{}`",
                    block.name, field.name
                ));
            }
        }
    }

    let metadata_path = root.join("crates/freehand-metadata/src/lib.rs");
    let metadata = fs::read_to_string(&metadata_path)
        .map_err(|err| format!("read metadata source {}: {err}", metadata_path.display()))?;
    for source_path in rust_source_paths(root)? {
        if source_path == metadata_path {
            continue;
        }
        let source = fs::read_to_string(&source_path)
            .map_err(|err| format!("read source file {}: {err}", source_path.display()))?;
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("pub struct Metadata")
                || trimmed.starts_with("pub enum Metadata")
            {
                return Err(format!(
                    "metadata owner type must stay inside crates/freehand-metadata: {}",
                    relative_slash_path(root, &source_path)?
                ));
            }
        }
    }

    for block in extract_struct_blocks(&metadata) {
        if !block.name.starts_with("Metadata") {
            continue;
        }
        for field in parse_struct_fields(&block.body) {
            if is_forbidden_metadata_field_name(field.name) {
                return Err(format!(
                    "metadata owner struct `{}` introduces forbidden request payload field `{}`",
                    block.name, field.name
                ));
            }
            if is_forbidden_metadata_field_type(field.ty) {
                return Err(format!(
                    "metadata owner struct `{}` introduces forbidden request payload type `{}` via field `{}`",
                    block.name, field.ty, field.name
                ));
            }
        }
    }

    Ok(())
}

fn split_binding_segments(value: &str) -> Vec<String> {
    value
        .split(" / ")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .map(str::to_owned)
        .collect()
}

fn symbol_resolves_in_files(
    root: &Path,
    file_paths: &[String],
    symbol: &str,
) -> Result<bool, String> {
    let candidates = symbol_lookup_candidates(symbol);
    for file_path in file_paths {
        let full_path = root.join(file_path);
        let text = fs::read_to_string(&full_path)
            .map_err(|err| format!("read source file {}: {err}", full_path.display()))?;
        if candidates
            .iter()
            .any(|candidate| text.contains(candidate.as_str()))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn rust_source_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
    let mut paths = Vec::new();
    for rel in ["crates", "apps"] {
        collect_rust_source_paths(&root.join(rel), &mut paths)?;
    }
    paths.sort();
    Ok(paths)
}

fn collect_rust_source_paths(dir: &Path, paths: &mut Vec<PathBuf>) -> Result<(), String> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|err| format!("read dir {}: {err}", dir.display()))? {
        let entry = entry.map_err(|err| err.to_string())?;
        let path = entry.path();
        if path.is_dir() {
            collect_rust_source_paths(&path, paths)?;
            continue;
        }
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            paths.push(path);
        }
    }
    Ok(())
}

fn extract_struct_blocks(source: &str) -> Vec<StructBlock<'_>> {
    let mut blocks = Vec::new();
    let mut lines = source.lines().enumerate().peekable();
    while let Some((index, line)) = lines.next() {
        let trimmed = line.trim();
        if !trimmed.starts_with("pub struct ") || !trimmed.contains('{') {
            continue;
        }

        let Some(after_prefix) = trimmed.strip_prefix("pub struct ") else {
            continue;
        };
        let Some(name) = after_prefix
            .split(|ch: char| ch == '{' || ch.is_whitespace())
            .find(|part| !part.is_empty())
        else {
            continue;
        };

        let body_start = index + 1;
        let mut body_end = body_start;
        let mut depth = line.matches('{').count() as i32 - line.matches('}').count() as i32;
        while depth > 0 {
            let Some((next_index, next_line)) = lines.next() else {
                break;
            };
            depth += next_line.matches('{').count() as i32;
            depth -= next_line.matches('}').count() as i32;
            body_end = next_index;
        }
        let body = source
            .lines()
            .skip(body_start)
            .take(body_end.saturating_sub(body_start))
            .collect::<Vec<_>>()
            .join("\n");
        blocks.push(StructBlock { name, body });
    }
    blocks
}

fn parse_struct_fields(body: &str) -> Vec<StructField<'_>> {
    body.lines()
        .filter_map(|line| {
            let trimmed = line.trim().trim_end_matches(',');
            if trimmed.is_empty()
                || trimmed.starts_with('#')
                || trimmed.starts_with("//")
                || !trimmed.contains(':')
            {
                return None;
            }
            let (name_part, ty_part) = trimmed.split_once(':')?;
            let name = name_part
                .trim()
                .strip_prefix("pub ")
                .unwrap_or(name_part.trim());
            let ty = ty_part.trim();
            if name.is_empty() || ty.is_empty() {
                return None;
            }
            Some(StructField { name, ty })
        })
        .collect()
}

fn is_forbidden_request_field_name(name: &str) -> bool {
    let normalized = name.trim().to_ascii_lowercase();
    normalized.contains("metadata")
        || normalized.contains("debug")
        || normalized.contains("control")
        || normalized.contains("routing")
        || normalized.contains("checkpoint")
        || normalized.contains("cancel")
        || normalized.contains("retry")
        || normalized.contains("gate")
        || matches!(
            normalized.as_str(),
            "cache_payload"
                | "cache_metadata"
                | "cache_debug"
                | "route_policy"
                | "routing_policy"
                | "rewrite_policy"
                | "execution_policy"
                | "control_policy"
                | "control_envelope"
                | "control_payload"
        )
}

fn is_forbidden_request_field_type(ty: &str) -> bool {
    [
        "Metadata",
        "Debug",
        "Control",
        "Routing",
        "Checkpoint",
        "Cancellation",
        "CancelToken",
        "RetryPolicy",
        "GateDecision",
        "UiCommand",
        "RuntimeCheckpoint",
    ]
    .iter()
    .any(|forbidden| ty.contains(forbidden))
}

fn is_forbidden_metadata_field_name(name: &str) -> bool {
    matches!(
        name.trim().to_ascii_lowercase().as_str(),
        "prompt"
            | "prompt_text"
            | "messages"
            | "message"
            | "message_text"
            | "user_text"
            | "context_segments"
            | "input_segments"
            | "tool_result"
            | "content"
            | "text"
            | "control_payload"
            | "control_envelope"
            | "routing_policy"
            | "route_policy"
            | "checkpoint_payload"
            | "cancel_token"
            | "retry_policy"
            | "gate_decision"
    )
}

fn is_forbidden_metadata_field_type(ty: &str) -> bool {
    ty.contains("ReasonReq")
        || ty.contains("ContextSegment")
        || ty.contains("ToolResultContract")
        || ty.contains("Routing")
        || ty.contains("RuntimeCheckpoint")
        || ty.contains("CancelToken")
        || ty.contains("RetryPolicy")
}

struct StructBlock<'a> {
    name: &'a str,
    body: String,
}

struct StructField<'a> {
    name: &'a str,
    ty: &'a str,
}

fn symbol_lookup_candidates(symbol: &str) -> Vec<String> {
    let mut candidates = vec![symbol.to_owned()];
    if let Some(last) = symbol
        .rsplit("::")
        .next()
        .filter(|last| *last != symbol && !last.is_empty())
    {
        candidates.push(last.to_owned());
    }
    candidates.sort();
    candidates.dedup();
    candidates
}

fn mainline_source_paths(root: &Path) -> Result<Vec<PathBuf>, String> {
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
    Ok(source_paths)
}

fn relative_slash_path(root: &Path, path: &Path) -> Result<String, String> {
    let relative = path.strip_prefix(root).map_err(|err| {
        format!(
            "path {} is not under repo root {}: {err}",
            path.display(),
            root.display()
        )
    })?;
    Ok(relative.to_string_lossy().replace('\\', "/"))
}

fn require_equal(
    actual: &str,
    expected: &str,
    feature_id: &str,
    field: &str,
) -> Result<(), String> {
    if actual != expected {
        return Err(format!(
            "mainline manifest `{feature_id}` has invalid {field}: expected `{expected}`, got `{actual}`"
        ));
    }
    Ok(())
}

fn require_contains(text: &str, snippet: &str, rel_path: &str) -> Result<(), String> {
    if !text.contains(snippet) {
        return Err(format!(
            "mainline manifest cross-link missing `{snippet}` in {rel_path}"
        ));
    }
    Ok(())
}

fn require_absent(text: &str, snippet: &str, rel_path: &str) -> Result<(), String> {
    if text.contains(snippet) {
        return Err(format!("{rel_path} must not contain `{snippet}`"));
    }
    Ok(())
}

fn resource_map_binding_section<'a>(
    function_map: &'a str,
    rel_path: &str,
) -> Result<&'a str, String> {
    function_map
        .split("## Resource Map Binding")
        .nth(1)
        .and_then(|tail| tail.split("\n## ").next())
        .ok_or_else(|| {
            format!("function map `{rel_path}` missing `## Resource Map Binding` section")
        })
}

fn require_function_map_binding_label_has_value(
    section: &str,
    label: &str,
    rel_path: &str,
) -> Result<(), String> {
    let marker = format!("- {label}:");
    let mut lines = section.lines().peekable();
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if !trimmed.starts_with(&marker) {
            continue;
        }
        let inline_value = trimmed[marker.len()..].trim();
        if !inline_value.is_empty() {
            return Ok(());
        }
        while let Some(next_line) = lines.peek() {
            let next_trimmed = next_line.trim();
            if next_trimmed.is_empty() {
                lines.next();
                continue;
            }
            if next_line.starts_with("  - ") && next_trimmed.len() > 2 {
                return Ok(());
            }
            break;
        }
        return Err(format!(
            "function map `{rel_path}` Resource Map Binding `{label}` must declare at least one value"
        ));
    }
    Err(format!(
        "function map `{rel_path}` Resource Map Binding missing `{label}:`"
    ))
}

fn require_resource_operation_test_coverage(
    root: &Path,
    test_design: &str,
    operation_id: &str,
    binding_status: &str,
    rel_path: &str,
) -> Result<(), String> {
    require_contains(test_design, "## Resource Operation Test Coverage", rel_path)?;
    require_contains(test_design, "white-box", rel_path)?;
    require_contains(test_design, "module black-box", rel_path)?;
    require_contains(test_design, "project black-box", rel_path)?;

    let operation_marker = format!("`{operation_id}`");
    let cells = test_design
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('|') && line.ends_with('|'))
        .map(|line| {
            line.trim_matches('|')
                .split('|')
                .map(str::trim)
                .collect::<Vec<_>>()
        })
        .find(|cells| cells.first().copied() == Some(operation_marker.as_str()))
        .ok_or_else(|| {
            format!(
                "test design `{rel_path}` must include a Resource Operation Test Coverage table row for `{operation_id}`"
            )
        })?;
    if cells.len() < 5 {
        return Err(format!(
            "test design `{rel_path}` Resource Operation Test Coverage row for `{operation_id}` must include operation, status, white-box, module black-box, and project black-box columns"
        ));
    }
    if cells[1] != binding_status {
        return Err(format!(
            "test design `{rel_path}` Resource Operation Test Coverage row for `{operation_id}` has status `{}`, expected `{binding_status}`",
            cells[1]
        ));
    }
    for (column_name, value) in [
        ("white-box", cells[2]),
        ("module black-box", cells[3]),
        ("project black-box", cells[4]),
    ] {
        if value.is_empty() || value == "-" {
            return Err(format!(
                "test design `{rel_path}` Resource Operation Test Coverage row for `{operation_id}` has empty {column_name} coverage"
            ));
        }
        if binding_status == "bound" && contains_pending_coverage_language(value) {
            return Err(format!(
                "test design `{rel_path}` Resource Operation Test Coverage row for bound `{operation_id}` has pending/future {column_name} coverage"
            ));
        }
        if binding_status == "bound" && !contains_verification_command(value) {
            return Err(format!(
                "test design `{rel_path}` Resource Operation Test Coverage row for bound `{operation_id}` {column_name} coverage must include a command-style verification entry"
            ));
        }
        if binding_status == "bound" {
            validate_coverage_command_entries(root, value, rel_path, operation_id, column_name)?;
        }
    }
    Ok(())
}

fn contains_pending_coverage_language(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    ["pending", "future", "not claimed", "not yet", "todo", "tbd"]
        .iter()
        .any(|needle| lowered.contains(needle))
}

fn contains_verification_command(value: &str) -> bool {
    [
        "`cargo ",
        "`make ",
        "`scripts/",
        "`bash ",
        "`node ",
        "`jq ",
        "`grep ",
        "`freehand-cli",
        "`./gradlew",
    ]
    .iter()
    .any(|needle| value.contains(needle))
}

fn validate_coverage_command_entries(
    root: &Path,
    value: &str,
    rel_path: &str,
    operation_id: &str,
    column_name: &str,
) -> Result<(), String> {
    let commands = extract_backtick_commands(value);
    if commands.is_empty() {
        return Err(format!(
            "test design `{rel_path}` Resource Operation Test Coverage row for bound `{operation_id}` {column_name} coverage must include a backticked command"
        ));
    }
    let package_names = cargo_package_names(root)?;
    let makefile = fs::read_to_string(root.join("Makefile")).unwrap_or_default();
    for command in commands {
        validate_coverage_command_entry(
            root,
            &package_names,
            &makefile,
            &command,
            rel_path,
            operation_id,
            column_name,
        )?;
    }
    Ok(())
}

fn extract_backtick_commands(value: &str) -> Vec<String> {
    let mut commands = Vec::new();
    let mut remaining = value;
    while let Some(start) = remaining.find('`') {
        let after_start = &remaining[start + 1..];
        let Some(end) = after_start.find('`') else {
            break;
        };
        let command = after_start[..end].trim();
        if is_verification_command_entry(command) {
            commands.push(command.to_owned());
        }
        remaining = &after_start[end + 1..];
    }
    commands
}

fn is_verification_command_entry(command: &str) -> bool {
    [
        "cargo ",
        "make ",
        "scripts/",
        "bash ",
        "node ",
        "jq ",
        "grep ",
        "freehand-cli",
        "./gradlew",
    ]
    .iter()
    .any(|prefix| command.starts_with(prefix))
}

fn validate_coverage_command_entry(
    root: &Path,
    package_names: &BTreeSet<String>,
    makefile: &str,
    command: &str,
    rel_path: &str,
    operation_id: &str,
    column_name: &str,
) -> Result<(), String> {
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err(format!(
            "test design `{rel_path}` Resource Operation Test Coverage row for bound `{operation_id}` {column_name} coverage has empty command entry"
        ));
    }
    match parts[0] {
        "cargo" => {
            if let Some(package) = command_package_arg(&parts)
                && !package_names.contains(package)
            {
                return Err(format!(
                    "test design `{rel_path}` Resource Operation Test Coverage row for bound `{operation_id}` {column_name} coverage references unknown cargo package `{package}`"
                ));
            }
        }
        "make" => {
            if let Some(target) = parts.get(1) {
                let target_marker = format!("{target}:");
                if !makefile.contains(&target_marker) {
                    return Err(format!(
                        "test design `{rel_path}` Resource Operation Test Coverage row for bound `{operation_id}` {column_name} coverage references unknown make target `{target}`"
                    ));
                }
            }
        }
        "scripts/verify-provider-retry-online.sh" | "scripts/verify-timer-tool-online.sh" => {
            if !root.join(parts[0]).is_file() {
                return Err(format!(
                    "test design `{rel_path}` Resource Operation Test Coverage row for bound `{operation_id}` {column_name} coverage references missing script `{}`",
                    parts[0]
                ));
            }
        }
        "bash" | "node" | "jq" | "grep" | "freehand-cliS" | "freehand-cli" | "./gradlew" => {}
        other if other.starts_with("scripts/") => {
            if !root.join(other).is_file() {
                return Err(format!(
                    "test design `{rel_path}` Resource Operation Test Coverage row for bound `{operation_id}` {column_name} coverage references missing script `{other}`"
                ));
            }
        }
        other => {
            return Err(format!(
                "test design `{rel_path}` Resource Operation Test Coverage row for bound `{operation_id}` {column_name} coverage uses unsupported command `{other}`"
            ));
        }
    }
    Ok(())
}

fn command_package_arg<'a>(parts: &'a [&'a str]) -> Option<&'a str> {
    parts.windows(2).find_map(|window| match window {
        ["-p" | "--package", package] => Some(*package),
        _ => None,
    })
}

fn cargo_package_names(root: &Path) -> Result<BTreeSet<String>, String> {
    let mut names = BTreeSet::new();
    collect_cargo_package_names(root, root, &mut names)?;
    Ok(names)
}

fn collect_cargo_package_names(
    root: &Path,
    dir: &Path,
    names: &mut BTreeSet<String>,
) -> Result<(), String> {
    let entries = fs::read_dir(dir)
        .map_err(|err| format!("read cargo package directory {}: {err}", dir.display()))?;
    for entry in entries {
        let entry = entry.map_err(|err| format!("read cargo package directory entry: {err}"))?;
        let path = entry.path();
        let file_type = entry
            .file_type()
            .map_err(|err| format!("read file type {}: {err}", path.display()))?;
        if file_type.is_dir() {
            let file_name = entry.file_name();
            if matches!(
                file_name.to_str(),
                Some("target" | ".git" | "artifacts" | "output" | "dist" | "node_modules")
            ) {
                continue;
            }
            collect_cargo_package_names(root, &path, names)?;
        } else if file_type.is_file() && entry.file_name() == "Cargo.toml" {
            let rel = relative_slash_path(root, &path)?;
            let text = fs::read_to_string(&path)
                .map_err(|err| format!("read cargo manifest {rel}: {err}"))?;
            if let Some(name) = parse_cargo_package_name(&text) {
                names.insert(name);
            }
        }
    }
    Ok(())
}

fn parse_cargo_package_name(manifest: &str) -> Option<String> {
    let mut in_package = false;
    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed == "[package]" {
            in_package = true;
            continue;
        }
        if trimmed.starts_with('[') {
            in_package = false;
            continue;
        }
        if !in_package || !trimmed.starts_with("name") {
            continue;
        }
        let (_, value) = trimmed.split_once('=')?;
        let name = value.trim().trim_matches('"');
        if !name.is_empty() {
            return Some(name.to_owned());
        }
    }
    None
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
    if !doc.resource_operations.is_empty() {
        render_bullets(
            &mut out,
            "Resource Operation Backlinks",
            &doc.resource_operations,
        );
    }
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
    out.push_str("| step | symbol path | file path | responsibility | input semantic | output semantic | caller | callee | source resource | target resource | resource operation | binding status |\n");
    out.push_str("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |\n");
    for row in &doc.call_table {
        let source_resource = row.source_resource.as_deref().unwrap_or("");
        let target_resource = row.target_resource.as_deref().unwrap_or("");
        let resource_operation = row.resource_operation.as_deref().unwrap_or("");
        out.push_str(&format!(
            "| {} | `{}` | `{}` | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            row.step,
            row.symbol_path,
            row.file_path,
            row.responsibility,
            row.input_semantic,
            row.output_semantic,
            row.caller,
            row.callee,
            source_resource,
            target_resource,
            resource_operation,
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
    #[serde(default)]
    resource_operations: Vec<String>,
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
    #[serde(default)]
    source_resource: Option<String>,
    #[serde(default)]
    target_resource: Option<String>,
    #[serde(default)]
    resource_operation: Option<String>,
    binding_status: String,
}

#[derive(Debug, Deserialize)]
struct ResourceMapDoc {
    schema_version: u32,
    #[allow(dead_code)]
    resource_map_id: String,
    resources: Vec<ResourceMapResource>,
    operation_bindings: Vec<ResourceMapOperationBinding>,
    #[serde(default)]
    source_edge_registry: Vec<ResourceMapSourceEdge>,
    relation_rules: Vec<ResourceMapRelationRule>,
    forbidden_direct_relations: Vec<ResourceMapForbiddenRelation>,
    #[serde(default)]
    source_shortcut_gates: Vec<ResourceMapSourceShortcutGate>,
    #[serde(default)]
    precise_source_edge_gates: Vec<ResourceMapPreciseSourceEdgeGate>,
}

#[derive(Debug, Deserialize)]
struct ResourceMapResource {
    resource_type: String,
    owner_feature_id: String,
    owner_crate: String,
    identity: String,
    truth_store: String,
    operations: Vec<String>,
    #[allow(dead_code)]
    projections: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ResourceMapOperationBinding {
    operation_id: String,
    owner_feature_id: String,
    source_resource: String,
    target_resource: String,
    #[allow(dead_code)]
    effect: String,
    mainline_call_doc: String,
    binding_status: String,
    #[serde(default)]
    pending_reason: Option<String>,
    #[serde(default)]
    pending_closure_doc: Option<String>,
    #[serde(default)]
    pending_verification: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ResourceMapSourceEdge {
    edge_id: String,
    operation_id: String,
    source_resource: String,
    target_resource: String,
    mainline_call_doc: String,
    call_table_step: String,
    file_path: String,
    symbol_path: String,
    binding_status: String,
}

#[derive(Debug, Deserialize)]
struct ResourceMapRelationRule {
    rule_id: String,
    source_resource: String,
    target_resource: String,
    allowed_direct: bool,
    via_resources: Vec<String>,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct ResourceMapForbiddenRelation {
    source_resource: String,
    target_resource: String,
    required_via: Vec<String>,
    source_gate_status: String,
    source_gate_reason: String,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct ResourceMapSourceShortcutGate {
    source_resource: String,
    target_resource: String,
    forbidden_packages: Vec<String>,
    forbidden_import_tokens: Vec<String>,
    reason: String,
}

#[derive(Debug, Deserialize)]
struct ResourceMapPreciseSourceEdgeGate {
    source_resource: String,
    target_resource: String,
    file_path: String,
    symbol_path: String,
    required_tokens: Vec<String>,
    forbidden_tokens: Vec<String>,
    reason: String,
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

struct ForbiddenDependencyEdge {
    from: &'static str,
    to: &'static str,
    reason: &'static str,
    baseline_violation: bool,
}

const FORBIDDEN_DEPENDENCY_EDGES: &[ForbiddenDependencyEdge] = &[
    ForbiddenDependencyEdge {
        from: "crates/freehand-reason",
        to: "freehand-ui-protocol",
        reason: "reason is a truth owner and must not build-depend on the UI contract surface",
        baseline_violation: false,
    },
    ForbiddenDependencyEdge {
        from: "crates/freehand-node",
        to: "freehand-ui-protocol",
        reason: "node internal state must not use UI contract types as its truth source",
        baseline_violation: false,
    },
    ForbiddenDependencyEdge {
        from: "apps/freehand-cli",
        to: "freehand-testkit",
        reason: "production binaries must not depend on test harness crates",
        baseline_violation: false,
    },
    ForbiddenDependencyEdge {
        from: "crates/freehand-runtime",
        to: "freehand-provider-openai",
        reason: "runtime must reach providers through freehand-provider-core, not concrete adapters",
        baseline_violation: false,
    },
    ForbiddenDependencyEdge {
        from: "crates/freehand-runtime",
        to: "freehand-provider-anthropic",
        reason: "runtime must reach providers through freehand-provider-core, not concrete adapters",
        baseline_violation: false,
    },
    ForbiddenDependencyEdge {
        from: "crates/freehand-testkit",
        to: "freehand-runtime",
        reason: "testkit harnesses build on reason/provider-core seams, not the runtime god crate",
        baseline_violation: false,
    },
    ForbiddenDependencyEdge {
        from: "crates/freehand-testkit",
        to: "freehand-config",
        reason: "declared but unused dependency edges must stay deleted",
        baseline_violation: false,
    },
    ForbiddenDependencyEdge {
        from: "crates/freehand-testkit",
        to: "freehand-provider-anthropic",
        reason: "declared but unused dependency edges must stay deleted",
        baseline_violation: false,
    },
];

fn cargo_dependencies_section(cargo: &str) -> String {
    let mut in_deps = false;
    let mut section = String::new();
    for line in cargo.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_deps = trimmed == "[dependencies]";
            continue;
        }
        if in_deps {
            section.push_str(line);
            section.push('\n');
        }
    }
    section
}

fn verify_adp_protocol_artifacts(root: &Path) -> Result<(), String> {
    use std::process::Command;

    let expected_json = root.join("crates/freehand-ui-protocol/generated/adp-protocol.schema.json");
    let expected_js = root.join("apps/freehand-server/assets/webui/generated/adp-protocol.js");
    let regenerated_json = env::temp_dir().join(format!(
        "freehand-adp-protocol-{}.schema.json",
        std::process::id()
    ));
    let regenerated_js =
        env::temp_dir().join(format!("freehand-adp-protocol-{}.js", std::process::id()));
    if !expected_json.is_file() {
        return Err(format!(
            "missing ADP protocol schema artifact: {}",
            expected_json.display()
        ));
    }
    if !expected_js.is_file() {
        return Err(format!(
            "missing ADP protocol WebUI constructor artifact: {}",
            expected_js.display()
        ));
    }

    let status = Command::new("cargo")
        .args([
            "run",
            "-q",
            "-p",
            "freehand-ui-protocol",
            "--bin",
            "export-adp-protocol",
            "--",
            "--json",
            regenerated_json
                .to_str()
                .ok_or("regenerated ADP schema path must be UTF-8")?,
        ])
        .current_dir(root)
        .status()
        .map_err(|err| format!("run export-adp-protocol --json: {err}"))?;
    if !status.success() {
        return Err("export-adp-protocol --json failed".to_owned());
    }
    let status = Command::new("cargo")
        .args([
            "run",
            "-q",
            "-p",
            "freehand-ui-protocol",
            "--bin",
            "export-adp-protocol",
            "--",
            "--js",
            regenerated_js
                .to_str()
                .ok_or("regenerated ADP WebUI module path must be UTF-8")?,
        ])
        .current_dir(root)
        .status()
        .map_err(|err| format!("run export-adp-protocol --js: {err}"))?;
    if !status.success() {
        return Err("export-adp-protocol --js failed".to_owned());
    }

    let expected_json_body = fs::read_to_string(&expected_json)
        .map_err(|err| format!("read {}: {err}", expected_json.display()))?;
    let expected_js_body = fs::read_to_string(&expected_js)
        .map_err(|err| format!("read {}: {err}", expected_js.display()))?;
    let actual_json_body = fs::read_to_string(&regenerated_json)
        .map_err(|err| format!("read regenerated ADP schema: {err}"))?;
    let actual_js_body = fs::read_to_string(&regenerated_js)
        .map_err(|err| format!("read regenerated ADP WebUI module: {err}"))?;
    if expected_json_body != actual_json_body {
        return Err(
            "ADP protocol schema artifact is stale; run `cargo run -p freehand-ui-protocol --bin export-adp-protocol -- --json crates/freehand-ui-protocol/generated/adp-protocol.schema.json`"
                .to_owned(),
        );
    }
    if expected_js_body != actual_js_body {
        return Err(
            "ADP protocol WebUI constructor artifact is stale; run `cargo run -p freehand-ui-protocol --bin export-adp-protocol -- --js apps/freehand-server/assets/webui/generated/adp-protocol.js`"
                .to_owned(),
        );
    }

    require_contains(
        &expected_json_body,
        "\"protocol_version\": 3",
        "crates/freehand-ui-protocol/generated/adp-protocol.schema.json",
    )?;
    require_contains(
        &expected_json_body,
        "\"handshake_capability\": \"adp.v3.handshake\"",
        "crates/freehand-ui-protocol/generated/adp-protocol.schema.json",
    )?;
    require_contains(
        &expected_json_body,
        "\"serde_name\": \"QueryConfigStatus\"",
        "crates/freehand-ui-protocol/generated/adp-protocol.schema.json",
    )?;
    require_contains(
        &expected_js_body,
        "export function adpQueryOf",
        "apps/freehand-server/assets/webui/generated/adp-protocol.js",
    )?;
    require_contains(
        &expected_js_body,
        "export function adpCommandOf",
        "apps/freehand-server/assets/webui/generated/adp-protocol.js",
    )?;
    require_absent(
        &expected_json_body,
        "target_owner_module",
        "crates/freehand-ui-protocol/generated/adp-protocol.schema.json",
    )?;
    require_absent(
        &expected_json_body,
        "crates/freehand-",
        "crates/freehand-ui-protocol/generated/adp-protocol.schema.json",
    )?;
    require_absent(
        &expected_js_body,
        "target_owner_module",
        "apps/freehand-server/assets/webui/generated/adp-protocol.js",
    )?;
    require_absent(
        &expected_js_body,
        "crates/freehand-",
        "apps/freehand-server/assets/webui/generated/adp-protocol.js",
    )?;

    let assets = fs::read_to_string(root.join("apps/freehand-server/src/assets.rs"))
        .map_err(|err| format!("read apps/freehand-server/src/assets.rs: {err}"))?;
    require_contains(
        &assets,
        "webui/generated/adp-protocol.js",
        "apps/freehand-server/src/assets.rs",
    )?;
    require_contains(
        &assets,
        "include_str!(\"../assets/webui/generated/adp-protocol.js\")",
        "apps/freehand-server/src/assets.rs",
    )?;

    let adp_client =
        fs::read_to_string(root.join("apps/freehand-server/assets/webui/app-shell/adp-client.js"))
            .map_err(|err| {
                format!("read apps/freehand-server/assets/webui/app-shell/adp-client.js: {err}")
            })?;
    require_contains(
        &adp_client,
        "generated/adp-protocol.js",
        "apps/freehand-server/assets/webui/app-shell/adp-client.js",
    )?;

    let legacy =
        fs::read_to_string(root.join("apps/freehand-server/assets/webui/legacy-monolith.js"))
            .map_err(|err| {
                format!("read apps/freehand-server/assets/webui/legacy-monolith.js: {err}")
            })?;
    require_contains(
        &legacy,
        "adpQueryOf",
        "apps/freehand-server/assets/webui/legacy-monolith.js",
    )?;
    require_contains(
        &legacy,
        "adpCommandOf",
        "apps/freehand-server/assets/webui/legacy-monolith.js",
    )?;
    Ok(())
}

fn verify_dependency_graph(root: &Path) -> Result<(), String> {
    for edge in FORBIDDEN_DEPENDENCY_EDGES {
        let cargo_path = root.join(edge.from).join("Cargo.toml");
        let cargo = fs::read_to_string(&cargo_path)
            .map_err(|err| format!("{}: {err}", cargo_path.display()))?;
        let deps = cargo_dependencies_section(&cargo);
        let has_edge = deps.lines().any(|line| {
            line.trim_start().starts_with(&format!("{} ", edge.to))
                || line.trim_start().starts_with(&format!("{}=", edge.to))
        });
        if has_edge && !edge.baseline_violation {
            return Err(format!(
                "forbidden dependency edge: {} -> {} ({})",
                edge.from, edge.to, edge.reason
            ));
        }
        if !has_edge && edge.baseline_violation {
            return Err(format!(
                "dependency baseline is stale: {} no longer depends on {}; flip baseline_violation to false in xtask FORBIDDEN_DEPENDENCY_EDGES so the edge stays locked",
                edge.from, edge.to
            ));
        }
    }
    Ok(())
}

/// Baseline of functions in freehand-task allowed to assign `task.status`
/// directly. The single-writer target is `mutate_task` (which routes through
/// `validate_transition`); `create_task`/`assign_task` must not assign status
/// directly. Any new direct assignment outside this list fails the gate.
const TASK_STATUS_WRITER_BASELINE: &[&str] = &["mutate_task", "apply_event"];

fn verify_task_status_single_writer(root: &Path) -> Result<(), String> {
    let path = root.join("crates/freehand-task/src/lib.rs");
    let source = fs::read_to_string(&path).map_err(|err| format!("{}: {err}", path.display()))?;
    let mut current_fn = String::new();
    let mut in_tests = false;
    for (idx, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with("mod tests") || trimmed.starts_with("#[cfg(test)]") {
            in_tests = true;
        }
        if let Some(rest) = trimmed
            .strip_prefix("pub fn ")
            .or_else(|| trimmed.strip_prefix("fn "))
            && let Some(name) = rest.split(['(', '<']).next()
        {
            current_fn = name.trim().to_owned();
        }
        if in_tests {
            continue;
        }
        let is_status_write = (trimmed.contains(".status = TaskStatus::")
            || trimmed.contains(".status = target"))
            && !trimmed.starts_with("//");
        if is_status_write && !TASK_STATUS_WRITER_BASELINE.contains(&current_fn.as_str()) {
            return Err(format!(
                "task status single-writer gate: `{}` at crates/freehand-task/src/lib.rs:{} assigns task.status directly; route the transition through mutate_task/validate_transition or extend TASK_STATUS_WRITER_BASELINE only with an architecture-gaps entry",
                current_fn,
                idx + 1
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn mainline_manifest_links_accept_aligned_docs() {
        let root = test_repo_root("aligned");
        write_mainline_fixture(&root, FixtureMode::Aligned);

        verify_mainline_manifest_links(&root).expect("aligned manifest links should pass");
    }

    #[test]
    fn mainline_manifest_links_reject_wrong_function_map_path() {
        let root = test_repo_root("wrong-function-map");
        write_mainline_fixture(&root, FixtureMode::WrongFunctionMapPath);

        let err = verify_mainline_manifest_links(&root).expect_err("wrong function map must fail");
        assert!(err.contains("invalid function_map_doc"), "{err}");
    }

    #[test]
    fn mainline_manifest_links_reject_missing_feature_map_link() {
        let root = test_repo_root("missing-feature-map-link");
        write_mainline_fixture(&root, FixtureMode::MissingFeatureMapLink);

        let err =
            verify_mainline_manifest_links(&root).expect_err("missing feature map link must fail");
        assert!(err.contains("docs/architecture/feature-map.md"), "{err}");
    }

    #[test]
    fn mainline_call_table_bindings_accept_method_tail_and_file_presence() {
        let root = test_repo_root("binding-pass");
        create_dirs(&root);
        fs::write(
            root.join("src/lib.rs"),
            "pub struct Demo;\nimpl Demo { pub fn run(&self) {} }\npub fn helper() {}\n",
        )
        .expect("write source");
        fs::write(
            root.join("docs/mainline-calls/demo.feature.json"),
            r#"{
  "feature_id": "demo.feature",
  "owner_crate": "demo",
  "owner_module": "demo/src/lib.rs",
  "function_map_doc": "docs/function-maps/demo.feature.md",
  "test_design_doc": "docs/testing/demo.feature.md",
  "mainline_call_doc": "docs/mainline-calls/demo.feature.json",
  "generated_wiki_doc": "docs/wiki/demo.feature.md",
  "request_mainline": [],
  "response_mainline": [],
  "error_mainline": [],
  "shared_functions": [],
  "call_table": [
    {
      "step": "01",
      "symbol_path": "Demo::run / helper",
      "file_path": "src/lib.rs",
      "responsibility": "demo",
      "input_semantic": "demo",
      "output_semantic": "demo",
      "caller": "demo",
      "callee": "demo",
      "binding_status": "bound"
    }
  ],
  "sync_status": []
}"#,
        )
        .expect("write mainline json");

        verify_mainline_call_table_bindings(&root)
            .expect("method tail and helper symbol should pass");
    }

    #[test]
    fn mainline_call_table_bindings_reject_missing_symbol() {
        let root = test_repo_root("binding-missing-symbol");
        create_dirs(&root);
        fs::write(root.join("src/lib.rs"), "pub fn present() {}\n").expect("write source");
        fs::write(
            root.join("docs/mainline-calls/demo.feature.json"),
            r#"{
  "feature_id": "demo.feature",
  "owner_crate": "demo",
  "owner_module": "demo/src/lib.rs",
  "function_map_doc": "docs/function-maps/demo.feature.md",
  "test_design_doc": "docs/testing/demo.feature.md",
  "mainline_call_doc": "docs/mainline-calls/demo.feature.json",
  "generated_wiki_doc": "docs/wiki/demo.feature.md",
  "request_mainline": [],
  "response_mainline": [],
  "error_mainline": [],
  "shared_functions": [],
  "call_table": [
    {
      "step": "01",
      "symbol_path": "missing_symbol",
      "file_path": "src/lib.rs",
      "responsibility": "demo",
      "input_semantic": "demo",
      "output_semantic": "demo",
      "caller": "demo",
      "callee": "demo",
      "binding_status": "bound"
    }
  ],
  "sync_status": []
}"#,
        )
        .expect("write mainline json");

        let err = verify_mainline_call_table_bindings(&root).expect_err("missing symbol must fail");
        assert!(err.contains("missing symbol"), "{err}");
    }

    #[test]
    fn ci_cd_gate_commands_accept_aligned_full_gate() {
        let root = test_repo_root("ci-cd-aligned");
        write_ci_cd_fixture(&root, CiFixtureMode::Aligned);

        verify_ci_cd_gate_commands(&root).expect("aligned CI/CD full gate should pass");
    }

    #[test]
    fn ci_cd_gate_commands_reject_make_ci_without_mainlines() {
        let root = test_repo_root("ci-cd-missing-mainlines");
        write_ci_cd_fixture(&root, CiFixtureMode::MakeCiMissingMainlines);

        let err = verify_ci_cd_gate_commands(&root)
            .expect_err("make ci without mainlines check must fail");
        assert!(err.contains("Makefile"), "{err}");
    }

    #[test]
    fn ci_cd_gate_commands_reject_ci_workflow_without_full_gate() {
        let root = test_repo_root("ci-cd-partial-ci");
        write_ci_cd_fixture(&root, CiFixtureMode::CiWorkflowPartialGate);

        let err =
            verify_ci_cd_gate_commands(&root).expect_err("CI workflow without make ci must fail");
        assert!(err.contains(".github/workflows/ci.yml"), "{err}");
    }

    #[test]
    fn ci_cd_gate_commands_reject_launchd_without_env_bind_health() {
        let root = test_repo_root("ci-cd-launchd-missing-env-bind");
        write_ci_cd_fixture(&root, CiFixtureMode::LaunchdMissingEnvBind);

        let err = verify_ci_cd_gate_commands(&root)
            .expect_err("launchd restart without env-backed health bind must fail");
        assert!(err.contains("scripts/install-launchd.sh"), "{err}");
    }

    #[test]
    fn ci_cd_gate_commands_reject_launchd_repo_root_master_workdir() {
        let root = test_repo_root("ci-cd-launchd-repo-root-workdir");
        write_ci_cd_fixture(&root, CiFixtureMode::LaunchdRepoRootWorkdir);

        let err = verify_ci_cd_gate_commands(&root)
            .expect_err("launchd repository-root master workdir must fail");
        assert!(err.contains("repository root"), "{err}");
    }

    #[test]
    fn feature_map_unique_entries_accept_single_seed_entry() {
        let root = test_repo_root("feature-map-unique");
        write_feature_map_fixture(&root, FeatureMapFixtureMode::Aligned);

        verify_feature_map_unique_entries(&root).expect("unique feature-map entries should pass");
    }

    #[test]
    fn feature_map_unique_entries_reject_duplicate_seed_entry() {
        let root = test_repo_root("feature-map-duplicate");
        write_feature_map_fixture(&root, FeatureMapFixtureMode::DuplicateSeedEntry);

        let err = verify_feature_map_unique_entries(&root)
            .expect_err("duplicate feature-map seed entry must fail");
        assert!(err.contains("demo.feature"), "{err}");
        assert!(err.contains("duplicate seed entry"), "{err}");
    }

    #[test]
    fn resource_map_accepts_registered_direct_edge() {
        let root = test_repo_root("resource-map-registered-edge");
        write_resource_map_fixture(&root, ResourceMapFixtureMode::Aligned);

        verify_resource_map(&root).expect("registered resource operation edge should pass");
    }

    #[test]
    fn resource_map_rejects_unregistered_direct_edge_row() {
        let root = test_repo_root("resource-map-unregistered-edge");
        write_resource_map_fixture(&root, ResourceMapFixtureMode::UnregisteredDirectEdgeRow);

        let err = verify_resource_map(&root)
            .expect_err("source/target resource row without resource_operation must fail");
        assert!(
            err.contains("declares source/target resource without resource_operation"),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_missing_source_edge_registry() {
        let root = test_repo_root("resource-map-missing-source-edge-registry");
        write_resource_map_fixture(&root, ResourceMapFixtureMode::MissingSourceEdgeRegistry);

        let err = verify_resource_map(&root)
            .expect_err("bound resource-operation rows must be in source_edge_registry");
        assert!(err.contains("missing from source_edge_registry"), "{err}");
    }

    #[test]
    fn resource_map_rejects_source_edge_registry_missing_symbol() {
        let root = test_repo_root("resource-map-source-edge-missing-symbol");
        write_resource_map_fixture(&root, ResourceMapFixtureMode::SourceEdgeMissingSymbol);

        let err = verify_resource_map(&root)
            .expect_err("source_edge_registry must bind to a real source symbol");
        assert!(
            err.contains(
                "source_edge_registry `demo.feature#01` references missing symbol `Demo::missing`"
            ),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_missing_direct_relation_rule() {
        let root = test_repo_root("resource-map-missing-direct-relation-rule");
        write_resource_map_fixture(&root, ResourceMapFixtureMode::MissingDirectRelationRule);

        let err = verify_resource_map(&root)
            .expect_err("bound resource-operation pairs must declare allowed_direct relation");
        assert!(
            err.contains("must have an allowed_direct relation rule"),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_operation_not_declared_on_source_resource() {
        let root = test_repo_root("resource-map-missing-allowed-operation");
        write_resource_map_fixture(
            &root,
            ResourceMapFixtureMode::MissingAllowedResourceOperation,
        );

        let err = verify_resource_map(&root)
            .expect_err("operation binding must be declared by source resource operations");
        assert!(
            err.contains("is not declared in resource `alpha` operations"),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_missing_feature_map_resource_backlink() {
        let root = test_repo_root("resource-map-missing-feature-backlink");
        write_resource_map_fixture(
            &root,
            ResourceMapFixtureMode::MissingFeatureMapResourceBacklink,
        );

        let err =
            verify_resource_map(&root).expect_err("feature map must backlink resource ownership");
        assert!(
            err.contains(
                "Resource Ownership Index owner `demo.feature` does not list resource `alpha`"
            ),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_unknown_feature_map_resource() {
        let root = test_repo_root("resource-map-unknown-feature-resource");
        write_resource_map_fixture(&root, ResourceMapFixtureMode::UnknownFeatureMapResource);

        let err = verify_resource_map(&root)
            .expect_err("feature map resource ownership must not list unknown resources");
        assert!(err.contains("lists unknown resource `ghost`"), "{err}");
    }

    #[test]
    fn resource_map_rejects_duplicate_feature_map_resource_owner() {
        let root = test_repo_root("resource-map-duplicate-feature-resource-owner");
        write_resource_map_fixture(
            &root,
            ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner,
        );

        let err = verify_resource_map(&root)
            .expect_err("feature map resource ownership must be unique by resource");
        assert!(err.contains("resource `alpha` is owned by both"), "{err}");
    }

    #[test]
    fn resource_map_rejects_feature_owner_crate_mismatch() {
        let root = test_repo_root("resource-map-feature-owner-crate-mismatch");
        write_resource_map_fixture(&root, ResourceMapFixtureMode::FeatureOwnerCrateMismatch);

        let err = verify_resource_map(&root)
            .expect_err("feature-map owner must contain resource owner_crate");
        assert!(
            err.contains("owner_crate `crates/demo` is not present in feature map owner"),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_missing_required_core_resource() {
        let root = test_repo_root("resource-map-missing-required-core-resource");
        write_resource_map_fixture(&root, ResourceMapFixtureMode::MissingRequiredCoreResource);

        let err = verify_resource_map(&root)
            .expect_err("core resource map must include required resources");
        assert!(
            err.contains("core resource map missing required resource `config`"),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_missing_resource_projection() {
        let root = test_repo_root("resource-map-missing-resource-projection");
        write_resource_map_fixture(&root, ResourceMapFixtureMode::MissingResourceProjection);

        let err =
            verify_resource_map(&root).expect_err("resources must declare at least one projection");
        assert!(
            err.contains("resource `alpha` must declare at least one projection"),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_empty_operation_binding_effect() {
        let root = test_repo_root("resource-map-empty-operation-binding-effect");
        write_resource_map_fixture(&root, ResourceMapFixtureMode::EmptyOperationBindingEffect);

        let err = verify_resource_map(&root).expect_err("operation bindings must describe effect");
        assert!(
            err.contains("`operation_bindings.effect` must not be empty"),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_pending_operation_missing_contract() {
        let root = test_repo_root("resource-map-pending-operation-missing-contract");
        write_resource_map_fixture(
            &root,
            ResourceMapFixtureMode::PendingOperationMissingContract,
        );

        let err =
            verify_resource_map(&root).expect_err("pending operations must declare closure truth");
        assert!(
            err.contains("`operation_bindings.pending_reason` must not be empty"),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_empty_relation_rule_reason() {
        let root = test_repo_root("resource-map-empty-relation-rule-reason");
        write_resource_map_fixture(&root, ResourceMapFixtureMode::EmptyRelationRuleReason);

        let err = verify_resource_map(&root).expect_err("relation rules must explain the relation");
        assert!(
            err.contains("`relation_rules.reason` must not be empty"),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_forbidden_allowed_direct_conflict() {
        let root = test_repo_root("resource-map-forbidden-allowed-direct-conflict");
        write_resource_map_fixture(
            &root,
            ResourceMapFixtureMode::ForbiddenAllowedDirectConflict,
        );

        let err = verify_resource_map(&root)
            .expect_err("forbidden direct relations must not conflict with allowed direct rules");
        assert!(
            err.contains(
                "forbidden direct relation alpha -> beta conflicts with an allowed_direct relation rule"
            ),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_noop_source_shortcut_gate() {
        let root = test_repo_root("resource-map-noop-source-shortcut-gate");
        write_resource_map_fixture(&root, ResourceMapFixtureMode::NoopSourceShortcutGate);

        let err = verify_resource_map(&root)
            .expect_err("source shortcut gates must declare at least one check");
        assert!(
            err.contains(
                "source shortcut gate `beta` -> `alpha` must declare forbidden_packages or forbidden_import_tokens"
            ),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_duplicate_source_shortcut_gate_pair() {
        let root = test_repo_root("resource-map-duplicate-source-shortcut-gate");
        write_resource_map_fixture(&root, ResourceMapFixtureMode::DuplicateSourceShortcutGate);

        let err =
            verify_resource_map(&root).expect_err("source shortcut gate pairs must be unique");
        assert!(
            err.contains("duplicate source_shortcut_gates pair `beta` -> `alpha`"),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_empty_forbidden_direct_relation_reason() {
        let root = test_repo_root("resource-map-empty-forbidden-direct-relation-reason");
        write_resource_map_fixture(
            &root,
            ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason,
        );

        let err = verify_resource_map(&root)
            .expect_err("forbidden direct relations must explain why direct access is forbidden");
        assert!(
            err.contains("`forbidden_direct_relations.reason` must not be empty"),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_forbidden_without_indirect_relation_rule() {
        let root = test_repo_root("resource-map-forbidden-missing-indirect-relation");
        write_resource_map_fixture(
            &root,
            ResourceMapFixtureMode::MissingForbiddenIndirectRelation,
        );

        let err = verify_resource_map(&root)
            .expect_err("forbidden direct relations must be backed by indirect relation rules");
        assert!(
            err.contains(
                "forbidden direct relation beta -> alpha must have a matching indirect relation rule"
            ),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_empty_function_map_resource_binding() {
        let root = test_repo_root("resource-map-empty-function-map-resource-binding");
        write_resource_map_fixture(
            &root,
            ResourceMapFixtureMode::EmptyFunctionMapResourceBinding,
        );

        let err = verify_resource_map(&root)
            .expect_err("function maps must declare non-empty resource binding lists");
        assert!(
            err.contains(
                "function map `docs/function-maps/demo.feature.md` Resource Map Binding `touched resources` must declare at least one value"
            ),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_pending_coverage_for_bound_operation() {
        let root = test_repo_root("resource-map-pending-coverage-for-bound-operation");
        write_resource_map_fixture(
            &root,
            ResourceMapFixtureMode::PendingCoverageForBoundOperation,
        );

        let err = verify_resource_map(&root)
            .expect_err("bound resource operations must not use pending coverage language");
        assert!(
            err.contains(
                "Resource Operation Test Coverage row for bound `alpha.to_beta` has pending/future project black-box coverage"
            ),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_operation_only_mentioned_in_wrong_coverage_cell() {
        let root = test_repo_root("resource-map-operation-mentioned-in-wrong-coverage-cell");
        write_resource_map_fixture(
            &root,
            ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell,
        );

        let err = verify_resource_map(&root)
            .expect_err("operation id must appear in the coverage operation cell");
        assert!(
            err.contains(
                "must include a Resource Operation Test Coverage table row for `alpha.to_beta`"
            ),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_bound_coverage_without_command_entry() {
        let root = test_repo_root("resource-map-bound-coverage-without-command-entry");
        write_resource_map_fixture(
            &root,
            ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry,
        );

        let err = verify_resource_map(&root)
            .expect_err("bound resource operation coverage must include command entries");
        assert!(
            err.contains("white-box coverage must include a command-style verification entry"),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_bound_coverage_unknown_cargo_package() {
        let root = test_repo_root("resource-map-bound-coverage-unknown-cargo-package");
        write_resource_map_fixture(
            &root,
            ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage,
        );

        let err = verify_resource_map(&root)
            .expect_err("bound coverage commands must reference known cargo packages");
        assert!(
            err.contains("references unknown cargo package `missing-package`"),
            "{err}"
        );
    }

    #[test]
    fn resource_map_rejects_duplicate_precise_source_edge_gate_pair() {
        let root = test_repo_root("resource-map-duplicate-precise-source-edge-gate");
        write_resource_map_fixture(
            &root,
            ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate,
        );

        let err =
            verify_resource_map(&root).expect_err("precise source edge gate pairs must be unique");
        assert!(
            err.contains("duplicate precise_source_edge_gates pair `beta` -> `alpha`"),
            "{err}"
        );
    }

    #[test]
    fn resource_source_gate_status_rejects_deferred() {
        parse_source_gate_status("checked", "demo relation")
            .expect("checked status should be accepted");
        parse_source_gate_status("precise_checked", "demo relation")
            .expect("precise_checked status should be accepted");

        let err = parse_source_gate_status("deferred", "demo relation")
            .expect_err("deferred status must fail");
        assert!(err.contains("unsupported source_gate_status"), "{err}");
        assert!(err.contains("deferred"), "{err}");
    }

    #[test]
    fn metadata_request_boundaries_accept_aligned_sources() {
        let root = test_repo_root("metadata-boundary-aligned");
        write_metadata_boundary_fixture(&root, MetadataBoundaryFixtureMode::Aligned);

        verify_data_control_boundaries(&root).expect("aligned data/control boundary");
    }

    #[test]
    fn metadata_request_boundaries_reject_request_metadata_type() {
        let root = test_repo_root("metadata-boundary-request-type");
        write_metadata_boundary_fixture(&root, MetadataBoundaryFixtureMode::RequestMetadataType);

        let err = verify_data_control_boundaries(&root)
            .expect_err("request metadata type leak must fail");
        assert!(err.contains("ReasonReq01UserRawInput"), "{err}");
        assert!(err.contains("MetadataEnvelope"), "{err}");
    }

    #[test]
    fn metadata_request_boundaries_reject_request_debug_field_name() {
        let root = test_repo_root("metadata-boundary-request-debug-field");
        write_metadata_boundary_fixture(&root, MetadataBoundaryFixtureMode::RequestDebugFieldName);

        let err =
            verify_data_control_boundaries(&root).expect_err("request debug field leak must fail");
        assert!(err.contains("debug_payload"), "{err}");
    }

    #[test]
    fn metadata_request_boundaries_reject_request_control_envelope_field() {
        let root = test_repo_root("metadata-boundary-request-control-field");
        write_metadata_boundary_fixture(
            &root,
            MetadataBoundaryFixtureMode::RequestControlEnvelopeField,
        );

        let err = verify_data_control_boundaries(&root)
            .expect_err("request control field leak must fail");
        assert!(err.contains("control_envelope"), "{err}");
    }

    #[test]
    fn metadata_request_boundaries_reject_stray_metadata_owner_type() {
        let root = test_repo_root("metadata-boundary-stray-owner");
        write_metadata_boundary_fixture(&root, MetadataBoundaryFixtureMode::StrayMetadataOwnerType);

        let err =
            verify_data_control_boundaries(&root).expect_err("stray metadata owner type must fail");
        assert!(err.contains("freehand-runtime/src/lib.rs"), "{err}");
    }

    #[test]
    fn metadata_request_boundaries_reject_metadata_prompt_field() {
        let root = test_repo_root("metadata-boundary-metadata-prompt-field");
        write_metadata_boundary_fixture(&root, MetadataBoundaryFixtureMode::MetadataPromptField);

        let err =
            verify_data_control_boundaries(&root).expect_err("metadata prompt field must fail");
        assert!(err.contains("MetadataEnvelope"), "{err}");
        assert!(err.contains("prompt_text"), "{err}");
    }

    #[test]
    fn metadata_request_boundaries_reject_metadata_request_payload_type() {
        let root = test_repo_root("metadata-boundary-metadata-request-type");
        write_metadata_boundary_fixture(&root, MetadataBoundaryFixtureMode::MetadataRequestType);

        let err = verify_data_control_boundaries(&root)
            .expect_err("metadata request payload type must fail");
        assert!(err.contains("MetadataEnvelope"), "{err}");
        assert!(err.contains("ContextSegment"), "{err}");
    }

    #[test]
    fn metadata_request_boundaries_reject_metadata_control_payload_type() {
        let root = test_repo_root("metadata-boundary-metadata-control-type");
        write_metadata_boundary_fixture(&root, MetadataBoundaryFixtureMode::MetadataControlType);

        let err = verify_data_control_boundaries(&root)
            .expect_err("metadata control payload type must fail");
        assert!(err.contains("MetadataEnvelope"), "{err}");
        assert!(err.contains("RuntimeCheckpoint"), "{err}");
    }

    #[test]
    fn source_search_policy_accepts_source_only_configuration() {
        let root = test_repo_root("source-search-policy-aligned");
        write_source_search_policy_fixture(&root, SourceSearchPolicyFixtureMode::Aligned);

        verify_source_search_policy(&root).expect("aligned source search policy should pass");
    }

    #[test]
    fn source_search_policy_rejects_missing_artifact_exclusion() {
        let root = test_repo_root("source-search-policy-missing-artifacts");
        write_source_search_policy_fixture(&root, SourceSearchPolicyFixtureMode::MissingArtifacts);

        let err =
            verify_source_search_policy(&root).expect_err("missing artifact exclusion must fail");
        assert!(err.contains("artifacts"), "{err}");
    }

    #[test]
    fn source_search_policy_rejects_missing_unsafe_arg_guard() {
        let root = test_repo_root("source-search-policy-missing-unsafe-arg-guard");
        write_source_search_policy_fixture(
            &root,
            SourceSearchPolicyFixtureMode::MissingUnsafeArgGuard,
        );

        let err = verify_source_search_policy(&root)
            .expect_err("missing unsafe argument guard must fail");
        assert!(err.contains("for arg in \"$@\"; do"), "{err}");
    }

    enum FixtureMode {
        Aligned,
        WrongFunctionMapPath,
        MissingFeatureMapLink,
    }

    enum CiFixtureMode {
        Aligned,
        MakeCiMissingMainlines,
        CiWorkflowPartialGate,
        LaunchdMissingEnvBind,
        LaunchdRepoRootWorkdir,
    }

    enum FeatureMapFixtureMode {
        Aligned,
        DuplicateSeedEntry,
    }

    enum MetadataBoundaryFixtureMode {
        Aligned,
        RequestMetadataType,
        RequestDebugFieldName,
        RequestControlEnvelopeField,
        StrayMetadataOwnerType,
        MetadataPromptField,
        MetadataRequestType,
        MetadataControlType,
    }

    enum SourceSearchPolicyFixtureMode {
        Aligned,
        MissingArtifacts,
        MissingUnsafeArgGuard,
    }

    enum ResourceMapFixtureMode {
        Aligned,
        UnregisteredDirectEdgeRow,
        MissingSourceEdgeRegistry,
        MissingDirectRelationRule,
        MissingAllowedResourceOperation,
        MissingFeatureMapResourceBacklink,
        UnknownFeatureMapResource,
        DuplicateFeatureMapResourceOwner,
        FeatureOwnerCrateMismatch,
        MissingRequiredCoreResource,
        MissingResourceProjection,
        EmptyOperationBindingEffect,
        PendingOperationMissingContract,
        EmptyRelationRuleReason,
        ForbiddenAllowedDirectConflict,
        NoopSourceShortcutGate,
        DuplicateSourceShortcutGate,
        EmptyForbiddenDirectRelationReason,
        MissingForbiddenIndirectRelation,
        EmptyFunctionMapResourceBinding,
        PendingCoverageForBoundOperation,
        OperationMentionedInWrongCoverageCell,
        BoundCoverageWithoutCommandEntry,
        BoundCoverageUnknownCargoPackage,
        SourceEdgeMissingSymbol,
        DuplicatePreciseSourceEdgeGate,
    }

    fn test_repo_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let root = env::temp_dir().join(format!(
            "freehand-xtask-{name}-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir_all(&root).expect("create temp repo");
        root
    }

    fn write_mainline_fixture(root: &Path, mode: FixtureMode) {
        create_dirs(root);
        let feature_id = "demo.feature";
        let function_map_doc = match mode {
            FixtureMode::WrongFunctionMapPath => "docs/function-maps/wrong.md",
            FixtureMode::Aligned | FixtureMode::MissingFeatureMapLink => {
                "docs/function-maps/demo.feature.md"
            }
        };
        let feature_map = match mode {
            FixtureMode::MissingFeatureMapLink => "# Feature Map\n",
            FixtureMode::Aligned | FixtureMode::WrongFunctionMapPath => {
                "- mainline_call_doc: `docs/mainline-calls/demo.feature.json`\n- generated_wiki_doc: `docs/wiki/demo.feature.md`\n"
            }
        };
        fs::write(root.join("docs/architecture/feature-map.md"), feature_map)
            .expect("write feature map");
        fs::write(
            root.join("docs/function-maps/demo.feature.md"),
            "- feature_id: `demo.feature`\n- mainline call source: `docs/mainline-calls/demo.feature.json`\n",
        )
        .expect("write function map");
        fs::write(
            root.join("docs/testing/demo.feature.md"),
            "- feature_id: `demo.feature`\n",
        )
        .expect("write test design");
        fs::write(root.join("docs/wiki/demo.feature.md"), "# generated\n").expect("write wiki");
        fs::write(
            root.join("docs/mainline-calls/demo.feature.json"),
            format!(
                r#"{{
  "feature_id": "{feature_id}",
  "owner_crate": "demo",
  "owner_module": "demo/src/lib.rs",
  "function_map_doc": "{function_map_doc}",
  "test_design_doc": "docs/testing/demo.feature.md",
  "mainline_call_doc": "docs/mainline-calls/demo.feature.json",
  "generated_wiki_doc": "docs/wiki/demo.feature.md",
  "request_mainline": [],
  "response_mainline": [],
  "error_mainline": [],
  "shared_functions": [],
  "call_table": [],
  "sync_status": []
}}"#
            ),
        )
        .expect("write mainline json");
    }

    fn write_ci_cd_fixture(root: &Path, mode: CiFixtureMode) {
        for rel in [".githooks", ".github/workflows", "scripts"] {
            fs::create_dir_all(root.join(rel)).expect("create ci fixture dir");
        }
        let makefile = match mode {
            CiFixtureMode::Aligned
            | CiFixtureMode::CiWorkflowPartialGate
            | CiFixtureMode::LaunchdMissingEnvBind
            | CiFixtureMode::LaunchdRepoRootWorkdir => {
                ".PHONY: build fmt clippy test mainlines gates ci verify-webui-online verify-webui-release-online release install-global install-symlink install-launchd install-launchdS install-worker-launchd install-worker-launchdS restart-launchd restart-launchdS restart-worker-launchd restart-worker-launchdS uninstall-launchd uninstall-launchdS uninstall-worker-launchd uninstall-worker-launchdS launchd-status launchd-statusS worker-launchd-status worker-launchd-statusS launchd-logs launchd-logsS worker-launchd-logs worker-launchd-logsS hooks\n\
build:\n\tcargo build --workspace\n\
fmt:\n\tcargo fmt --check\n\
clippy:\n\tcargo clippy --workspace --all-targets -- -D warnings\n\
test:\n\tcargo test --workspace\n\
mainlines:\n\tcargo run -p xtask -- mainlines check\n\
gates:\n\tcargo run -p xtask -- gates check\n\
ci: build fmt clippy test mainlines gates\n\
verify-webui-online:\n\tscripts/verify-webui-online.sh\n\
verify-webui-release-online:\n\tscripts/verify-webui-release-online.sh\n\
release:\n\tscripts/release.sh\n\
install-global:\n\tscripts/install-global.sh\n\
install-symlink:\n\tscripts/install-symlink.sh\n\
install-launchd:\n\tscripts/install-launchd.sh\n\
install-launchdS:\n\tscripts/install-launchd.sh installS\n\
install-worker-launchdS:\n\tscripts/install-launchd.sh installWorkerS\n\
restart-launchd:\n\tscripts/install-launchd.sh restart\n\
restart-launchdS:\n\tscripts/install-launchd.sh restartS\n\
restart-worker-launchdS:\n\tscripts/install-launchd.sh restartWorkerS\n\
uninstall-launchd:\n\tscripts/uninstall-launchd.sh\n\
uninstall-launchdS:\n\tscripts/uninstall-launchd.sh uninstallS\n\
uninstall-worker-launchdS:\n\tscripts/uninstall-launchd.sh uninstallWorkerS\n"
            }
            CiFixtureMode::MakeCiMissingMainlines => {
                ".PHONY: build fmt clippy test gates ci verify-webui-online verify-webui-release-online release install-global install-symlink install-launchd install-launchdS install-worker-launchd install-worker-launchdS restart-launchd restart-launchdS restart-worker-launchd restart-worker-launchdS uninstall-launchd uninstall-launchdS uninstall-worker-launchd uninstall-worker-launchdS launchd-status launchd-statusS worker-launchd-status worker-launchd-statusS launchd-logs launchd-logsS worker-launchd-logs worker-launchd-logsS hooks\n\
build:\n\tcargo build --workspace\n\
fmt:\n\tcargo fmt --check\n\
clippy:\n\tcargo clippy --workspace --all-targets -- -D warnings\n\
test:\n\tcargo test --workspace\n\
gates:\n\tcargo run -p xtask -- gates check\n\
ci: build fmt clippy test gates\n\
verify-webui-online:\n\tscripts/verify-webui-online.sh\n\
verify-webui-release-online:\n\tscripts/verify-webui-release-online.sh\n\
release:\n\tscripts/release.sh\n\
install-global:\n\tscripts/install-global.sh\n\
install-symlink:\n\tscripts/install-symlink.sh\n\
install-launchd:\n\tscripts/install-launchd.sh\n\
install-launchdS:\n\tscripts/install-launchd.sh installS\n\
install-worker-launchdS:\n\tscripts/install-launchd.sh installWorkerS\n\
restart-launchd:\n\tscripts/install-launchd.sh restart\n\
restart-launchdS:\n\tscripts/install-launchd.sh restartS\n\
restart-worker-launchdS:\n\tscripts/install-launchd.sh restartWorkerS\n\
uninstall-launchd:\n\tscripts/uninstall-launchd.sh\n\
uninstall-launchdS:\n\tscripts/uninstall-launchd.sh uninstallS\n\
uninstall-worker-launchdS:\n\tscripts/uninstall-launchd.sh uninstallWorkerS\n"
            }
        };
        fs::write(root.join("Makefile"), makefile).expect("write Makefile fixture");
        let launchd_script = match mode {
            CiFixtureMode::LaunchdMissingEnvBind => {
                concat!(
                    "#!/usr/bin/env bash\n\
runtime_home=\"$HOME/.freehand\"\n\
logs_dir=\"$runtime_home/logs\"\n\
workdir=\"${FREEHAND_DAEMON_WORKDIR:-\"$runtime_home\"}\"\n\
mkdir -p \"$runtime_home\" \"$logs_dir\" \"$workdir\"\n\
default_daemon_bind() {\n\
  local port=\"$1\"\n\
  local profile_suffix=\"${2:-}\"\n\
  if [[ \"$profile_suffix\" == \"S\" ]]; then\n\
    printf '127.0.0.1:%s\\n' \"$port\"\n\
    return 0\n\
  fi\n\
}\n\
bind_addr=\"$default_bind_addr\"\n",
                    "installWorkerS|restartWorkerS)\n\
default_label=\"com.freehand.workerS\"\n\
exec \"$daemon_bin\" serve --agent \"$agent\"</string>\n\
echo \"worker requires FREEHAND_PAIR_TOKEN_SHARED\"\n\
copy_worker_provider_env_from_master() {\n\
  [[ \"$key\" =~ ^FREEHAND_.*(_KEY|CREDENTIAL|SECRET)$ ]]\n\
}\n\
wait_for_worker_service\n\
kill -0 \"$service_pid\"\n",
                )
            }
            CiFixtureMode::LaunchdRepoRootWorkdir => {
                concat!(
                    "#!/usr/bin/env bash\n\
runtime_home=\"$HOME/.freehand\"\n\
logs_dir=\"$runtime_home/logs\"\n\
workdir=\"${FREEHAND_DAEMON_WORKDIR:-\"$repo_root\"}\"\n\
mkdir -p \"$runtime_home\" \"$logs_dir\" \"$workdir\"\n\
default_daemon_bind() {\n\
  local port=\"$1\"\n\
  local profile_suffix=\"${2:-}\"\n\
  if [[ \"$profile_suffix\" == \"S\" ]]; then\n\
    printf '127.0.0.1:%s\\n' \"$port\"\n\
    return 0\n\
  fi\n\
}\n\
bind_addr=\"$default_bind_addr\"\n\
if [[ -n \"${FREEHAND_DAEMON_BIND:-}\" ]]; then\n\
  bind_addr=\"$FREEHAND_DAEMON_BIND\"\n\
elif [[ -f \"$env_file\" ]]; then\n\
  env_bind=\"$(awk -F= '$1 == \"FREEHAND_DAEMON_BIND\" { print $2; exit }' \"$env_file\")\"\n\
fi\n",
                    "set -a; [ -f \"$env_file\" ] && . \"$env_file\"; set +a;\n",
                    "restartS)\n\
    env -u FREEHAND_DAEMON_WORKDIR -u FREEHAND_WORKSPACE_ROOT scripts/install-symlink.sh\n\
    write_launchd_env\n\
    write_launchd_plist\n\
    launchctl bootout \"gui/$(id -u)\" \"$plist_path\"\n\
    launchctl bootstrap \"gui/$(id -u)\" \"$plist_path\"\n\
    restart_launchd\n",
                    "installWorkerS|restartWorkerS)\n\
default_label=\"com.freehand.workerS\"\n\
exec \"$daemon_bin\" serve --agent \"$agent\"</string>\n\
echo \"worker requires FREEHAND_PAIR_TOKEN_SHARED\"\n\
copy_worker_provider_env_from_master() {\n\
  [[ \"$key\" =~ ^FREEHAND_.*(_KEY|CREDENTIAL|SECRET)$ ]]\n\
}\n\
wait_for_worker_service\n\
kill -0 \"$service_pid\"\n",
                )
            }
            CiFixtureMode::Aligned
            | CiFixtureMode::MakeCiMissingMainlines
            | CiFixtureMode::CiWorkflowPartialGate => {
                concat!(
                    "#!/usr/bin/env bash\n\
runtime_home=\"$HOME/.freehand\"\n\
logs_dir=\"$runtime_home/logs\"\n\
workdir=\"${FREEHAND_DAEMON_WORKDIR:-\"$runtime_home\"}\"\n\
mkdir -p \"$runtime_home\" \"$logs_dir\" \"$workdir\"\n\
default_daemon_bind() {\n\
  local port=\"$1\"\n\
  local profile_suffix=\"${2:-}\"\n\
  if [[ \"$profile_suffix\" == \"S\" ]]; then\n\
    printf '127.0.0.1:%s\\n' \"$port\"\n\
    return 0\n\
  fi\n\
}\n\
bind_addr=\"$default_bind_addr\"\n\
if [[ -n \"${FREEHAND_DAEMON_BIND:-}\" ]]; then\n\
  bind_addr=\"$FREEHAND_DAEMON_BIND\"\n\
elif [[ -f \"$env_file\" ]]; then\n\
  env_bind=\"$(awk -F= '$1 == \"FREEHAND_DAEMON_BIND\" { print $2; exit }' \"$env_file\")\"\n\
fi\n",
                    "set -a; [ -f \"$env_file\" ] && . \"$env_file\"; set +a;\n",
                    "restartS)\n\
    env -u FREEHAND_DAEMON_WORKDIR -u FREEHAND_WORKSPACE_ROOT scripts/install-symlink.sh\n\
    write_launchd_env\n\
    write_launchd_plist\n\
    launchctl bootout \"gui/$(id -u)\" \"$plist_path\"\n\
    launchctl bootstrap \"gui/$(id -u)\" \"$plist_path\"\n\
    restart_launchd\n",
                    "installWorkerS|restartWorkerS)\n\
default_label=\"com.freehand.workerS\"\n\
exec \"$daemon_bin\" serve --agent \"$agent\"</string>\n\
echo \"worker requires FREEHAND_PAIR_TOKEN_SHARED\"\n\
copy_worker_provider_env_from_master() {\n\
  [[ \"$key\" =~ ^FREEHAND_.*(_KEY|CREDENTIAL|SECRET)$ ]]\n\
}\n\
wait_for_worker_service\n\
kill -0 \"$service_pid\"\n",
                )
            }
        };
        fs::write(root.join("scripts/install-launchd.sh"), launchd_script)
            .expect("write install launchd fixture");
        fs::write(
            root.join("scripts/verify-webui-online.sh"),
            "#!/usr/bin/env bash\n\
base_url=\"${FREEHAND_WEBUI_BASE_URL:-http://127.0.0.1:4042/}\"\n\
health_url=\"${FREEHAND_WEBUI_HEALTH_URL:-http://127.0.0.1:4042/health}\"\n\
adp_url=\"${FREEHAND_WEBUI_ADP_URL:-ws://127.0.0.1:4042/adp}\"\n\
cli_path=\"${FREEHAND_WEBUI_CLI:-$HOME/.local/bin/freehand-cliS}\"\n\
profile=\"${FREEHAND_WEBUI_PROFILE:-4042}\"\n",
        )
        .expect("write WebUI online fixture");
        fs::write(
            root.join("scripts/verify-webui-release-online.sh"),
            "#!/usr/bin/env bash\n\
FREEHAND_WEBUI_BASE_URL=\"${FREEHAND_WEBUI_BASE_URL:-http://127.0.0.1:4041/}\" \\\n\
FREEHAND_WEBUI_HEALTH_URL=\"${FREEHAND_WEBUI_HEALTH_URL:-http://127.0.0.1:4041/health}\" \\\n\
FREEHAND_WEBUI_ADP_URL=\"${FREEHAND_WEBUI_ADP_URL:-ws://127.0.0.1:4041/adp}\" \\\n\
FREEHAND_WEBUI_CLI=\"${FREEHAND_WEBUI_CLI:-$HOME/.local/bin/freehand-cli}\" \\\n\
FREEHAND_WEBUI_PROFILE=\"${FREEHAND_WEBUI_PROFILE:-4041}\" \\\n\
  scripts/verify-webui-online.sh\n",
        )
        .expect("write WebUI release online fixture");
        fs::write(
            root.join(".githooks/pre-push"),
            "#!/usr/bin/env bash\nset -euo pipefail\nmake ci\n",
        )
        .expect("write pre-push fixture");
        let ci_workflow = match mode {
            CiFixtureMode::Aligned
            | CiFixtureMode::MakeCiMissingMainlines
            | CiFixtureMode::LaunchdMissingEnvBind
            | CiFixtureMode::LaunchdRepoRootWorkdir => {
                "name: ci\njobs:\n  rust-gates:\n    steps:\n      - name: Full gate\n        run: make ci\n"
            }
            CiFixtureMode::CiWorkflowPartialGate => {
                "name: ci\njobs:\n  rust-gates:\n    steps:\n      - name: Architecture gates\n        run: cargo run -p xtask -- gates check\n"
            }
        };
        fs::write(root.join(".github/workflows/ci.yml"), ci_workflow)
            .expect("write ci workflow fixture");
        fs::write(
            root.join(".github/workflows/release.yml"),
            "name: release\njobs:\n  release:\n    steps:\n      - name: Full gate\n        run: make ci\n      - name: Build release artifacts\n        run: scripts/release.sh\n",
        )
        .expect("write release workflow fixture");
    }

    fn write_feature_map_fixture(root: &Path, mode: FeatureMapFixtureMode) {
        create_dirs(root);
        let feature_map = match mode {
            FeatureMapFixtureMode::Aligned => {
                "## Seed Entries\n\n### `demo.feature`\n\n- owner: `demo`\n".to_owned()
            }
            FeatureMapFixtureMode::DuplicateSeedEntry => {
                "## Seed Entries\n\n### `demo.feature`\n\n- owner: `demo`\n\n### `demo.feature`\n\n- owner: `demo-again`\n".to_owned()
            }
        };
        fs::write(root.join("docs/architecture/feature-map.md"), feature_map)
            .expect("write feature-map fixture");
    }

    fn write_resource_map_fixture(root: &Path, mode: ResourceMapFixtureMode) {
        create_dirs(root);
        fs::create_dir_all(root.join("docs/resource-maps")).expect("create resource-map dir");
        fs::create_dir_all(root.join("crates/demo/src")).expect("create demo crate dir");
        fs::write(
            root.join("crates/demo/src/lib.rs"),
            "pub struct Demo;\nimpl Demo { pub fn run(&self) {} }\n",
        )
        .expect("write demo source");
        let (feature_resource_cell, extra_feature_resource_rows) = match mode {
            ResourceMapFixtureMode::MissingFeatureMapResourceBacklink => ("`beta`", ""),
            ResourceMapFixtureMode::UnknownFeatureMapResource => ("`alpha`, `beta`, `ghost`", ""),
            ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner => (
                "`alpha`, `beta`",
                "\n| `demo.other` | `alpha` | `docs/resource-maps/core.json` |",
            ),
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::UnregisteredDirectEdgeRow
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::PendingCoverageForBoundOperation
            | ResourceMapFixtureMode::PendingOperationMissingContract
            | ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell
            | ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry
            | ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => ("`alpha`, `beta`", ""),
        };
        let feature_owner = match mode {
            ResourceMapFixtureMode::FeatureOwnerCrateMismatch => "`crates/other`",
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::UnregisteredDirectEdgeRow
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::PendingCoverageForBoundOperation
            | ResourceMapFixtureMode::PendingOperationMissingContract
            | ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell
            | ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry
            | ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => "`crates/demo`",
        };
        fs::write(
            root.join("docs/architecture/feature-map.md"),
            format!(
                "## Resource Ownership Index\n\n| feature_id | owned resources | resource map |\n| --- | --- | --- |\n| `demo.feature` | {feature_resource_cell} | `docs/resource-maps/core.json` |{extra_feature_resource_rows}\n\n## Seed Entries\n\n### `demo.feature`\n\n- owner: {feature_owner}\n- mainline_call_doc: `docs/mainline-calls/demo.feature.json`\n- generated_wiki_doc: `docs/wiki/demo.feature.md`\n"
            ),
        )
        .expect("write feature map");
        let function_map_doc = match mode {
            ResourceMapFixtureMode::EmptyFunctionMapResourceBinding => {
                "- feature_id: `demo.feature`\n- mainline call source: `docs/mainline-calls/demo.feature.json`\n\n## Resource Map Binding\n\n- resource map: `docs/resource-maps/core.json`\n- owned resources: `alpha`\n- touched resources:\n- resource operations: `alpha.to_beta`\n- forbidden shortcuts: none\n"
            }
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::UnregisteredDirectEdgeRow
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::PendingOperationMissingContract
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => {
                "- feature_id: `demo.feature`\n- mainline call source: `docs/mainline-calls/demo.feature.json`\n\n## Resource Map Binding\n\n- resource map: `docs/resource-maps/core.json`\n- owned resources: `alpha`\n- touched resources: `beta`\n- resource operations: `alpha.to_beta`\n- forbidden shortcuts: none\n"
            }
            ResourceMapFixtureMode::PendingCoverageForBoundOperation => {
                "- feature_id: `demo.feature`\n- mainline call source: `docs/mainline-calls/demo.feature.json`\n\n## Resource Map Binding\n\n- resource map: `docs/resource-maps/core.json`\n- owned resources: `alpha`\n- touched resources: `beta`\n- resource operations: `alpha.to_beta`\n- forbidden shortcuts: none\n"
            }
            ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell => {
                "- feature_id: `demo.feature`\n- mainline call source: `docs/mainline-calls/demo.feature.json`\n\n## Resource Map Binding\n\n- resource map: `docs/resource-maps/core.json`\n- owned resources: `alpha`\n- touched resources: `beta`\n- resource operations: `alpha.to_beta`\n- forbidden shortcuts: none\n"
            }
            ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry => {
                "- feature_id: `demo.feature`\n- mainline call source: `docs/mainline-calls/demo.feature.json`\n\n## Resource Map Binding\n\n- resource map: `docs/resource-maps/core.json`\n- owned resources: `alpha`\n- touched resources: `beta`\n- resource operations: `alpha.to_beta`\n- forbidden shortcuts: none\n"
            }
            ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage => {
                "- feature_id: `demo.feature`\n- mainline call source: `docs/mainline-calls/demo.feature.json`\n\n## Resource Map Binding\n\n- resource map: `docs/resource-maps/core.json`\n- owned resources: `alpha`\n- touched resources: `beta`\n- resource operations: `alpha.to_beta`\n- forbidden shortcuts: none\n"
            }
        };
        fs::write(
            root.join("docs/function-maps/demo.feature.md"),
            function_map_doc,
        )
        .expect("write function map");
        let project_black_box = match mode {
            ResourceMapFixtureMode::PendingCoverageForBoundOperation => {
                "pending: future project smoke"
            }
            ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry => "project smoke",
            ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage => {
                "`cargo test -p missing-package -- --nocapture`"
            }
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::UnregisteredDirectEdgeRow
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell
            | ResourceMapFixtureMode::PendingOperationMissingContract
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => {
                "`cargo run -p xtask -- gates check`"
            }
        };
        let coverage_operation_cell = match mode {
            ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell => "`alpha.other`",
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::UnregisteredDirectEdgeRow
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::PendingCoverageForBoundOperation
            | ResourceMapFixtureMode::PendingOperationMissingContract
            | ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry
            | ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => "`alpha.to_beta`",
        };
        let module_black_box = match mode {
            ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell => {
                "boundary smoke mentions `alpha.to_beta` in the wrong cell"
            }
            ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry => "boundary smoke",
            ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage => {
                "`cargo test -p xtask resource_map_ -- --nocapture`"
            }
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::UnregisteredDirectEdgeRow
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::PendingCoverageForBoundOperation
            | ResourceMapFixtureMode::PendingOperationMissingContract
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => {
                "`cargo test -p xtask resource_map_ -- --nocapture`"
            }
        };
        let white_box = match mode {
            ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry => "owner unit test",
            ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage => {
                "`cargo test -p xtask resource_map_ -- --nocapture`"
            }
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::UnregisteredDirectEdgeRow
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::PendingCoverageForBoundOperation
            | ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell
            | ResourceMapFixtureMode::PendingOperationMissingContract
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => {
                "`cargo test -p xtask resource_map_ -- --nocapture`"
            }
        };
        fs::write(
            root.join("docs/testing/demo.feature.md"),
            format!(
                "- feature_id: `demo.feature`\n- resource map: `docs/resource-maps/core.json`\n- resource operation coverage:\n  - `alpha.to_beta`\n\n## Resource Operation Test Coverage\n\n| resource operation | status | white-box | module black-box | project black-box |\n| --- | --- | --- | --- | --- |\n| {coverage_operation_cell} | bound | {white_box} | {module_black_box} | {project_black_box} |\n"
            ),
        )
        .expect("write test design");
        let resource_operation = match mode {
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::PendingCoverageForBoundOperation
            | ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell
            | ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry
            | ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage
            | ResourceMapFixtureMode::PendingOperationMissingContract
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => {
                r#",
      "resource_operation": "alpha.to_beta""#
            }
            ResourceMapFixtureMode::UnregisteredDirectEdgeRow => "",
        };
        let mainline_resource_operations = match mode {
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::PendingCoverageForBoundOperation
            | ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell
            | ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry
            | ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage
            | ResourceMapFixtureMode::PendingOperationMissingContract
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => {
                r#""resource_operations": ["alpha.to_beta"],"#
            }
            ResourceMapFixtureMode::UnregisteredDirectEdgeRow => r#""resource_operations": [],"#,
        };
        let source_edge_symbol_path = match mode {
            ResourceMapFixtureMode::SourceEdgeMissingSymbol => "Demo::missing",
            _ => "Demo::run",
        };
        fs::write(
            root.join("docs/mainline-calls/demo.feature.json"),
            format!(
                r#"{{
  "feature_id": "demo.feature",
  "owner_crate": "crates/demo",
  "owner_module": "crates/demo/src/lib.rs",
  "function_map_doc": "docs/function-maps/demo.feature.md",
  "test_design_doc": "docs/testing/demo.feature.md",
  "mainline_call_doc": "docs/mainline-calls/demo.feature.json",
  "generated_wiki_doc": "docs/wiki/demo.feature.md",
  {mainline_resource_operations}
  "request_mainline": [],
  "response_mainline": [],
  "error_mainline": [],
  "shared_functions": [],
  "call_table": [
    {{
      "step": "01",
      "symbol_path": "{source_edge_symbol_path}",
      "file_path": "crates/demo/src/lib.rs",
      "responsibility": "demo",
      "input_semantic": "alpha",
      "output_semantic": "beta",
      "caller": "demo",
      "callee": "demo",
      "source_resource": "alpha",
      "target_resource": "beta"{resource_operation},
      "binding_status": "bound"
    }}
  ],
  "sync_status": []
}}"#
            ),
        )
        .expect("write mainline");
        let operation_bindings = match mode {
            ResourceMapFixtureMode::PendingOperationMissingContract => r#"[
    {
      "operation_id": "alpha.to_beta",
      "owner_feature_id": "demo.feature",
      "source_resource": "alpha",
      "target_resource": "beta",
      "effect": "fixture pending edge without closure contract",
      "mainline_call_doc": "docs/mainline-calls/demo.feature.json",
      "binding_status": "pending"
    }
  ]"#
            .to_owned(),
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::PendingCoverageForBoundOperation
            | ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell
            | ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry
            | ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => {
                if matches!(mode, ResourceMapFixtureMode::EmptyOperationBindingEffect) {
                    r#"[
    {
      "operation_id": "alpha.to_beta",
      "owner_feature_id": "demo.feature",
      "source_resource": "alpha",
      "target_resource": "beta",
      "effect": "",
      "mainline_call_doc": "docs/mainline-calls/demo.feature.json",
      "binding_status": "bound"
    }
  ]"#
                    .to_owned()
                } else {
                    r#"[
    {
      "operation_id": "alpha.to_beta",
      "owner_feature_id": "demo.feature",
      "source_resource": "alpha",
      "target_resource": "beta",
      "effect": "fixture edge",
      "mainline_call_doc": "docs/mainline-calls/demo.feature.json",
      "binding_status": "bound"
    }
  ]"#
                    .to_owned()
                }
            }
            ResourceMapFixtureMode::UnregisteredDirectEdgeRow => "[]".to_owned(),
        };
        let source_edge_registry = match mode {
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::PendingCoverageForBoundOperation
            | ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell
            | ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry
            | ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => {
                format!(
                    r#"[
    {{
      "edge_id": "demo.feature#01",
      "operation_id": "alpha.to_beta",
      "source_resource": "alpha",
      "target_resource": "beta",
      "mainline_call_doc": "docs/mainline-calls/demo.feature.json",
      "call_table_step": "01",
      "file_path": "crates/demo/src/lib.rs",
      "symbol_path": "{source_edge_symbol_path}",
      "binding_status": "bound"
    }}
  ]"#
                )
            }
            ResourceMapFixtureMode::UnregisteredDirectEdgeRow
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::PendingOperationMissingContract => "[]".to_owned(),
        };
        let relation_rules = match mode {
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::UnregisteredDirectEdgeRow
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::PendingCoverageForBoundOperation
            | ResourceMapFixtureMode::PendingOperationMissingContract
            | ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell
            | ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry
            | ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => {
                if matches!(mode, ResourceMapFixtureMode::EmptyRelationRuleReason) {
                    r#"[
    {
      "rule_id": "alpha-to-beta-direct",
      "source_resource": "alpha",
      "target_resource": "beta",
      "allowed_direct": true,
      "via_resources": [],
      "reason": ""
    }
  ]"#
                    .to_owned()
                } else if matches!(mode, ResourceMapFixtureMode::NoopSourceShortcutGate) {
                    r#"[
    {
      "rule_id": "alpha-to-beta-direct",
      "source_resource": "alpha",
      "target_resource": "beta",
      "allowed_direct": true,
      "via_resources": [],
      "reason": "fixture direct relation"
    },
    {
      "rule_id": "beta-to-alpha-indirect",
      "source_resource": "beta",
      "target_resource": "alpha",
      "allowed_direct": false,
      "via_resources": ["alpha"],
      "reason": "fixture forbidden shortcut must route through alpha"
    }
  ]"#
                    .to_owned()
                } else {
                    r#"[
    {
      "rule_id": "alpha-to-beta-direct",
      "source_resource": "alpha",
      "target_resource": "beta",
      "allowed_direct": true,
      "via_resources": [],
      "reason": "fixture direct relation"
    }
  ]"#
                    .to_owned()
                }
            }
            ResourceMapFixtureMode::MissingDirectRelationRule => "[]".to_owned(),
        };
        let alpha_operations = match mode {
            ResourceMapFixtureMode::MissingAllowedResourceOperation => r#""read""#,
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::UnregisteredDirectEdgeRow
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::PendingCoverageForBoundOperation
            | ResourceMapFixtureMode::PendingOperationMissingContract
            | ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell
            | ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry
            | ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => r#""to_beta""#,
        };
        let alpha_projections = match mode {
            ResourceMapFixtureMode::MissingResourceProjection => "",
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::UnregisteredDirectEdgeRow
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingRequiredCoreResource
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::PendingCoverageForBoundOperation
            | ResourceMapFixtureMode::PendingOperationMissingContract
            | ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell
            | ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry
            | ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => r#""alpha projection""#,
        };
        let resource_map_id = match mode {
            ResourceMapFixtureMode::MissingRequiredCoreResource => "freehand.core-resource-map",
            ResourceMapFixtureMode::Aligned
            | ResourceMapFixtureMode::UnregisteredDirectEdgeRow
            | ResourceMapFixtureMode::MissingSourceEdgeRegistry
            | ResourceMapFixtureMode::MissingDirectRelationRule
            | ResourceMapFixtureMode::MissingAllowedResourceOperation
            | ResourceMapFixtureMode::MissingFeatureMapResourceBacklink
            | ResourceMapFixtureMode::UnknownFeatureMapResource
            | ResourceMapFixtureMode::DuplicateFeatureMapResourceOwner
            | ResourceMapFixtureMode::FeatureOwnerCrateMismatch
            | ResourceMapFixtureMode::MissingResourceProjection
            | ResourceMapFixtureMode::EmptyOperationBindingEffect
            | ResourceMapFixtureMode::EmptyRelationRuleReason
            | ResourceMapFixtureMode::ForbiddenAllowedDirectConflict
            | ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate
            | ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason
            | ResourceMapFixtureMode::MissingForbiddenIndirectRelation
            | ResourceMapFixtureMode::EmptyFunctionMapResourceBinding
            | ResourceMapFixtureMode::PendingCoverageForBoundOperation
            | ResourceMapFixtureMode::PendingOperationMissingContract
            | ResourceMapFixtureMode::OperationMentionedInWrongCoverageCell
            | ResourceMapFixtureMode::BoundCoverageWithoutCommandEntry
            | ResourceMapFixtureMode::BoundCoverageUnknownCargoPackage
            | ResourceMapFixtureMode::SourceEdgeMissingSymbol
            | ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => "fixture",
        };
        let forbidden_direct_relations = match mode {
            ResourceMapFixtureMode::ForbiddenAllowedDirectConflict => {
                r#"[
    {
      "source_resource": "alpha",
      "target_resource": "beta",
      "required_via": ["beta"],
      "reason": "fixture conflicting forbidden direct relation",
      "source_gate_status": "checked",
      "source_gate_reason": "fixture would require a source gate if it did not conflict first"
    }
  ]"#
            }
            ResourceMapFixtureMode::NoopSourceShortcutGate
            | ResourceMapFixtureMode::DuplicateSourceShortcutGate => {
                r#"[
    {
      "source_resource": "beta",
      "target_resource": "alpha",
      "required_via": ["alpha"],
      "reason": "fixture forbidden relation for noop source shortcut gate",
      "source_gate_status": "checked",
      "source_gate_reason": "fixture requires source shortcut gate coverage"
    }
  ]"#
            }
            ResourceMapFixtureMode::EmptyForbiddenDirectRelationReason => {
                r#"[
    {
      "source_resource": "beta",
      "target_resource": "alpha",
      "required_via": ["alpha"],
      "reason": "",
      "source_gate_status": "checked",
      "source_gate_reason": "fixture requires source shortcut gate coverage"
    }
  ]"#
            }
            ResourceMapFixtureMode::MissingForbiddenIndirectRelation => {
                r#"[
    {
      "source_resource": "beta",
      "target_resource": "alpha",
      "required_via": ["alpha"],
      "reason": "fixture forbidden relation without matching indirect relation rule",
      "source_gate_status": "checked",
      "source_gate_reason": "fixture should fail before source shortcut gate validation"
    }
  ]"#
            }
            ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => {
                r#"[
    {
      "source_resource": "beta",
      "target_resource": "alpha",
      "required_via": ["alpha"],
      "reason": "fixture forbidden relation for duplicate precise source edge gate",
      "source_gate_status": "precise_checked",
      "source_gate_reason": "fixture requires precise source edge gate coverage"
    }
  ]"#
            }
            _ => "[]",
        };
        let source_shortcut_gates = match mode {
            ResourceMapFixtureMode::NoopSourceShortcutGate => {
                r#"[
    {
      "source_resource": "beta",
      "target_resource": "alpha",
      "forbidden_packages": [],
      "forbidden_import_tokens": [],
      "reason": "fixture noop source shortcut gate"
    }
  ]"#
            }
            ResourceMapFixtureMode::DuplicateSourceShortcutGate => {
                r#"[
    {
      "source_resource": "beta",
      "target_resource": "alpha",
      "forbidden_packages": ["freehand-ghost"],
      "forbidden_import_tokens": [],
      "reason": "fixture duplicate source shortcut gate first"
    },
    {
      "source_resource": "beta",
      "target_resource": "alpha",
      "forbidden_packages": ["freehand-other-ghost"],
      "forbidden_import_tokens": [],
      "reason": "fixture duplicate source shortcut gate second"
    }
  ]"#
            }
            _ => "[]",
        };
        let precise_source_edge_gates = match mode {
            ResourceMapFixtureMode::DuplicatePreciseSourceEdgeGate => {
                r#"[
    {
      "source_resource": "beta",
      "target_resource": "alpha",
      "file_path": "crates/demo/src/lib.rs",
      "symbol_path": "Demo::run",
      "required_tokens": ["run"],
      "forbidden_tokens": [],
      "reason": "fixture duplicate precise source edge gate first"
    },
    {
      "source_resource": "beta",
      "target_resource": "alpha",
      "file_path": "crates/demo/src/lib.rs",
      "symbol_path": "Demo::run",
      "required_tokens": ["run"],
      "forbidden_tokens": [],
      "reason": "fixture duplicate precise source edge gate second"
    }
  ]"#
            }
            _ => "[]",
        };
        fs::write(
            root.join("docs/resource-maps/core.json"),
            format!(
                r#"{{
  "schema_version": 1,
  "resource_map_id": "{resource_map_id}",
  "resources": [
    {{
      "resource_type": "alpha",
      "owner_feature_id": "demo.feature",
      "owner_crate": "crates/demo",
      "identity": "alpha id",
      "truth_store": "alpha truth",
      "operations": [{alpha_operations}],
      "projections": [{alpha_projections}]
    }},
    {{
      "resource_type": "beta",
      "owner_feature_id": "demo.feature",
      "owner_crate": "crates/demo",
      "identity": "beta id",
      "truth_store": "beta truth",
      "operations": ["read"],
      "projections": ["beta projection"]
    }}
  ],
  "operation_bindings": {operation_bindings},
  "source_edge_registry": {source_edge_registry},
  "relation_rules": {relation_rules},
  "forbidden_direct_relations": {forbidden_direct_relations},
  "source_shortcut_gates": {source_shortcut_gates},
  "precise_source_edge_gates": {precise_source_edge_gates}
}}"#
            ),
        )
        .expect("write resource map");
    }

    fn create_dirs(root: &Path) {
        for rel in [
            "src",
            "xtask/src",
            "docs/architecture",
            "docs/function-maps",
            "docs/testing",
            "docs/wiki",
            "docs/mainline-calls",
        ] {
            fs::create_dir_all(root.join(rel)).expect("create fixture dir");
        }
        fs::write(
            root.join("xtask/Cargo.toml"),
            "[package]\nname = \"xtask\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
        )
        .expect("write xtask cargo fixture");
        fs::write(root.join("xtask/src/main.rs"), "fn main() {}\n")
            .expect("write xtask source fixture");
        fs::write(
            root.join("Makefile"),
            "verify-webui-online:\n\tscripts/verify-webui-online.sh\n",
        )
        .expect("write Makefile fixture");
    }

    fn write_metadata_boundary_fixture(root: &Path, mode: MetadataBoundaryFixtureMode) {
        for rel in [
            "crates/freehand-contracts/src",
            "crates/freehand-metadata/src",
            "crates/freehand-runtime/src",
            "apps/freehand-server/src",
            "xtask/src",
        ] {
            fs::create_dir_all(root.join(rel)).expect("create metadata boundary fixture dir");
        }

        let request_extra = match mode {
            MetadataBoundaryFixtureMode::Aligned
            | MetadataBoundaryFixtureMode::StrayMetadataOwnerType
            | MetadataBoundaryFixtureMode::MetadataPromptField
            | MetadataBoundaryFixtureMode::MetadataRequestType
            | MetadataBoundaryFixtureMode::MetadataControlType => String::new(),
            MetadataBoundaryFixtureMode::RequestMetadataType => {
                "    pub metadata: MetadataEnvelope,\n".to_owned()
            }
            MetadataBoundaryFixtureMode::RequestDebugFieldName => {
                "    pub debug_payload: String,\n".to_owned()
            }
            MetadataBoundaryFixtureMode::RequestControlEnvelopeField => {
                "    pub control_envelope: String,\n".to_owned()
            }
        };
        fs::write(
            root.join("crates/freehand-contracts/src/lib.rs"),
            format!(
                "pub struct ContextSegment {{\n    pub content: String,\n}}\n\n\
pub struct ReasonReq01UserRawInput {{\n    pub session_id: String,\n    pub text: String,\n{request_extra}}}\n\n\
pub struct ReasonReq02ContextComposedInput {{\n    pub user_text: String,\n    pub context_segments: Vec<ContextSegment>,\n}}\n"
            ),
        )
        .expect("write contracts fixture");

        let metadata_envelope_extra = match mode {
            MetadataBoundaryFixtureMode::Aligned
            | MetadataBoundaryFixtureMode::RequestMetadataType
            | MetadataBoundaryFixtureMode::RequestDebugFieldName
            | MetadataBoundaryFixtureMode::RequestControlEnvelopeField
            | MetadataBoundaryFixtureMode::StrayMetadataOwnerType => String::new(),
            MetadataBoundaryFixtureMode::MetadataPromptField => {
                "    prompt_text: String,\n".to_owned()
            }
            MetadataBoundaryFixtureMode::MetadataRequestType => {
                "    segments: Vec<ContextSegment>,\n".to_owned()
            }
            MetadataBoundaryFixtureMode::MetadataControlType => {
                "    checkpoint: RuntimeCheckpoint,\n".to_owned()
            }
        };
        fs::write(
            root.join("crates/freehand-metadata/src/lib.rs"),
            format!(
                "pub struct MetadataId(String);\n\n\
pub struct RuntimeCheckpoint {{\n    id: String,\n}}\n\n\
pub struct MetadataEnvelope {{\n    entries: Vec<String>,\n{metadata_envelope_extra}}}\n\n\
pub struct MetadataCenter {{\n    records: Vec<MetadataEnvelope>,\n}}\n"
            ),
        )
        .expect("write metadata fixture");

        let runtime_source = match mode {
            MetadataBoundaryFixtureMode::StrayMetadataOwnerType => {
                "pub struct MetadataLeak {\n    pub id: String,\n}\n"
            }
            _ => "pub struct RuntimeOk;\n",
        };
        fs::write(
            root.join("crates/freehand-runtime/src/lib.rs"),
            runtime_source,
        )
        .expect("write runtime fixture");

        fs::write(
            root.join("apps/freehand-server/src/lib.rs"),
            "pub fn app() {}\n",
        )
        .expect("write app fixture");
        fs::write(root.join("xtask/src/lib.rs"), "pub fn helper() {}\n")
            .expect("write xtask fixture");
    }

    fn write_source_search_policy_fixture(root: &Path, mode: SourceSearchPolicyFixtureMode) {
        for rel in [
            "scripts",
            ".agents/skills/freehand-dev",
            "docs/architecture",
        ] {
            fs::create_dir_all(root.join(rel)).expect("create source search fixture dir");
        }

        let artifact_ignore = match mode {
            SourceSearchPolicyFixtureMode::Aligned
            | SourceSearchPolicyFixtureMode::MissingUnsafeArgGuard => "artifacts/\n",
            SourceSearchPolicyFixtureMode::MissingArtifacts => "",
        };
        let artifact_glob = match mode {
            SourceSearchPolicyFixtureMode::Aligned
            | SourceSearchPolicyFixtureMode::MissingUnsafeArgGuard => {
                "  \"--glob=!artifacts/**\"\n"
            }
            SourceSearchPolicyFixtureMode::MissingArtifacts => "",
        };
        let unsafe_arg_guard = match mode {
            SourceSearchPolicyFixtureMode::Aligned
            | SourceSearchPolicyFixtureMode::MissingArtifacts => {
                "for arg in \"$@\"; do\n  case \"$arg\" in\n    --no-ignore|--unrestricted)\n      exit 2\n      ;;\n  esac\ndone\n"
            }
            SourceSearchPolicyFixtureMode::MissingUnsafeArgGuard => "",
        };
        fs::write(
            root.join(".ignore"),
            format!(
                "target/\ndist/\n{artifact_ignore}docs/wiki/\n.mempalace/\nmemory/*-mempalace-corpus/\ntest-palaces/\n**/build/\n**/.gradle/\n**/node_modules/\n"
            ),
        )
        .expect("write .ignore fixture");
        fs::write(
            root.join("scripts/source-search.sh"),
            format!(
                "#!/usr/bin/env bash\nreadonly -a exclude_globs=(\n  \"--glob=!target/**\"\n  \"--glob=!dist/**\"\n{artifact_glob}  \"--glob=!docs/wiki/**\"\n  \"--glob=!.mempalace/**\"\n  \"--glob=!memory/*-mempalace-corpus/**\"\n  \"--glob=!test-palaces/**\"\n)\n{unsafe_arg_guard}readonly -a search_roots=(docs/architecture docs/function-maps docs/mainline-calls docs/testing crates apps xtask)\nexec rg --hidden \"$@\" \"${{exclude_globs[@]}}\" \"${{search_roots[@]}}\"\n"
            ),
        )
        .expect("write source-search fixture");
        fs::write(
            root.join(".agents/skills/freehand-dev/SKILL.md"),
            "Debug/search truth is source-first.\nDo not search generated or runtime output when locating implementation truth.\nGenerated artifacts may be opened only as verification evidence.\nUse scripts/source-search.sh.\n",
        )
        .expect("write skill fixture");
        fs::write(
            root.join("docs/architecture/dev-debug-workflow.md"),
            "## Source-Only Search Rule\nUse scripts/source-search.sh. Generated outputs are evidence, not as implementation search roots.\n",
        )
        .expect("write debug workflow fixture");
        fs::write(
            root.join("docs/architecture/dev-gates.md"),
            "## Source Search Boundary Gate\n`xtask gates check` validates source-only search policy so generated outputs remain excluded from default implementation search.\n",
        )
        .expect("write dev gates fixture");
    }
}
