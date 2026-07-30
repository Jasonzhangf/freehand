use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::{require_contains, split_binding_segments};

const OPENMINIS_UI_MIGRATION_NODE_IDS: &[&str] = &[
    "foundation.root",
    "foundation.surface_contract",
    "foundation.protocol_calls",
    "foundation.shared_states",
    "home.dashboard",
    "session_detail.root",
    "session_detail.header",
    "session_detail.transcript",
    "session_detail.composer",
    "session_detail.agent_sheet",
    "session_search.root",
    "new_session.root",
    "turn_blocks.user",
    "turn_blocks.assistant",
    "turn_blocks.reasoning",
    "turn_blocks.tool_activity",
    "turn_blocks.attachment",
    "turn_blocks.artifact",
    "turn_blocks.error",
    "composer.text_submit",
    "composer.attachments",
    "composer.queue",
    "composer.stop_continue",
    "composer.voice",
    "tools.registry",
    "tools.detail",
    "tools.activity",
    "tools.permissions",
    "settings.root",
    "settings.models",
    "settings.agent_runtime",
    "settings.connection",
    "settings.observability",
    "settings.appearance",
    "settings.about",
    "timer.dashboard",
    "files_artifacts.root",
    "skills.root",
    "memory.root",
    "integrations.root",
    "platform.android_bridge",
];

const OPENMINIS_UI_MIGRATION_NODE_PREFIXES: &[&str] = &[
    "foundation.",
    "home.",
    "session_detail.",
    "session_search.",
    "new_session.",
    "turn_blocks.",
    "composer.",
    "tools.",
    "settings.",
    "timer.",
    "files_artifacts.",
    "skills.",
    "memory.",
    "integrations.",
    "platform.",
];

const OPENMINIS_UI_EXCLUDED_SOURCE_TOKENS: &[&str] = &[
    "browseruse",
    "browsersheet",
    "browsermanagement",
    "browsertab",
    "browsertool",
    "browsersnapshot",
    "cookie",
    "takeover",
    "profile",
];

const OPENMINIS_UI_EXCLUDED_SOURCE_PATH_TOKENS: &[&str] = &[
    "/browser/",
    "/browseruse/",
    "browseruse",
    "browsermanagement",
    "browsersheet",
    "browsertab",
    "cookie",
    "takeover",
    "profile",
];

const OPENMINIS_UI_REQUIRED_EXCLUDED_SOURCE_SYMBOLS: &[&str] = &[
    "BrowserSheetView",
    "BrowserManagementView",
    "BrowserUseManager",
    "BrowserTabPool",
    "BrowserUseOffloadBridge",
    "browserTool",
    "browserSnapshot",
    "cookieStore",
    "CookieBackupStore",
];

const OPENMINIS_UI_MANIFEST_STATUSES: &[&str] = &[
    "design_baseline",
    "migration_in_progress",
    "migration_complete",
];

const OPENMINIS_UI_MIGRATION_STATUSES: &[&str] = &[
    "inventoried",
    "owner_mapped",
    "contract_ready",
    "implementation_in_progress",
    "source_bound",
    "online_verified",
    "legacy_retired",
    "blocked_resource_missing",
    "blocked_owner_missing",
    "blocked_protocol_missing",
    "blocked_verification_missing",
];

pub(super) fn verify_openminis_ui_migration_manifest(root: &Path) -> Result<(), String> {
    let manifest_path = root.join("docs/migrations/openminis-ui/ui-tree.manifest.json");
    let raw = fs::read_to_string(&manifest_path)
        .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
    let manifest: Value =
        serde_json::from_str(&raw).map_err(|err| format!("{}: {err}", manifest_path.display()))?;
    verify_openminis_ui_migration_manifest_value(root, &manifest)
}

fn verify_openminis_ui_migration_manifest_value(
    root: &Path,
    manifest: &Value,
) -> Result<(), String> {
    let object = manifest
        .as_object()
        .ok_or_else(|| "OpenMinis UI migration manifest must be an object".to_owned())?;
    let manifest_status = required_string(object, "status", "OpenMinis UI migration manifest")?;
    if !OPENMINIS_UI_MANIFEST_STATUSES.contains(&manifest_status) {
        return Err(format!(
            "OpenMinis UI migration manifest has unknown status `{manifest_status}`"
        ));
    }
    if object.get("browser_included").and_then(Value::as_bool) != Some(false) {
        return Err("OpenMinis UI migration manifest must exclude browser scope".to_owned());
    }
    let source_repository = object
        .get("source_repository")
        .and_then(Value::as_object)
        .ok_or_else(|| "OpenMinis UI migration manifest missing source_repository".to_owned())?;
    if source_repository
        .get("repository_id")
        .and_then(Value::as_str)
        != Some("OpenMinis")
    {
        return Err("OpenMinis UI migration source_repository id must be OpenMinis".to_owned());
    }
    let source_commit = source_repository
        .get("commit")
        .and_then(Value::as_str)
        .ok_or_else(|| "OpenMinis UI migration source_repository missing commit".to_owned())?;
    if source_commit.len() != 40 || !source_commit.chars().all(|ch| ch.is_ascii_hexdigit()) {
        return Err(
            "OpenMinis UI migration source_repository commit must be a full 40-character git SHA"
                .to_owned(),
        );
    }
    let excluded_source_symbols = string_array(
        object.get("excluded_source_symbols"),
        "OpenMinis UI migration excluded_source_symbols",
    )?;
    let required_excluded_source_symbols = OPENMINIS_UI_REQUIRED_EXCLUDED_SOURCE_SYMBOLS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<BTreeSet<_>>();
    if excluded_source_symbols != required_excluded_source_symbols {
        return Err(format!(
            "OpenMinis UI migration excluded_source_symbols drift: expected {required_excluded_source_symbols:?}, got {excluded_source_symbols:?}"
        ));
    }

    let declared_required = string_array(
        object.get("required_node_ids"),
        "OpenMinis UI migration required_node_ids",
    )?;
    let expected_required = OPENMINIS_UI_MIGRATION_NODE_IDS
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<BTreeSet<_>>();
    if declared_required != expected_required {
        return Err(format!(
            "OpenMinis UI migration required_node_ids drift: expected {expected_required:?}, got {declared_required:?}"
        ));
    }

    let allowed_statuses = string_array(
        object.get("allowed_statuses"),
        "OpenMinis UI migration allowed_statuses",
    )?;
    let expected_statuses = OPENMINIS_UI_MIGRATION_STATUSES
        .iter()
        .map(|value| (*value).to_owned())
        .collect::<BTreeSet<_>>();
    if allowed_statuses != expected_statuses {
        return Err(format!(
            "OpenMinis UI migration allowed_statuses drift: expected {expected_statuses:?}, got {allowed_statuses:?}"
        ));
    }

    let nodes = object
        .get("nodes")
        .and_then(Value::as_array)
        .ok_or_else(|| "OpenMinis UI migration manifest missing nodes".to_owned())?;
    let feature_map = fs::read_to_string(root.join("docs/architecture/feature-map.md"))
        .map_err(|err| format!("read feature map for OpenMinis UI migration gate: {err}"))?;
    let resource_map_raw = fs::read_to_string(root.join("docs/resource-maps/core.json"))
        .map_err(|err| format!("read resource map for OpenMinis UI migration gate: {err}"))?;
    let resource_map: Value = serde_json::from_str(&resource_map_raw)
        .map_err(|err| format!("parse resource map for OpenMinis UI migration gate: {err}"))?;
    let operation_bindings = resource_map
        .get("operation_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| "resource map missing operation_bindings".to_owned())?;
    let mut node_ids = BTreeSet::new();
    for node in nodes {
        let node = node
            .as_object()
            .ok_or_else(|| "OpenMinis UI migration nodes must contain only objects".to_owned())?;
        let node_id = required_string(node, "node_id", "OpenMinis UI migration node")?;
        if !node_ids.insert(node_id.to_owned()) {
            return Err(format!(
                "OpenMinis UI migration manifest has duplicate node_id `{node_id}`"
            ));
        }
        let migration_unit_id = required_string(
            node,
            "migration_unit_id",
            &format!("migration node {node_id}"),
        )?;
        if migration_unit_id != format!("ui_migration.{node_id}") {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` has invalid migration_unit_id `{migration_unit_id}`"
            ));
        }
        let status = required_string(node, "status", &format!("migration node {node_id}"))?;
        if !allowed_statuses.contains(status) {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` has unknown status `{status}`"
            ));
        }
        verify_openminis_ui_lifecycle_fields(node_id, status, node)?;
        let owner_feature_id = required_string(
            node,
            "owner_feature_id",
            &format!("migration node {node_id}"),
        )?;
        if !feature_map.contains(&format!("`{owner_feature_id}`")) {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` references unknown owner feature `{owner_feature_id}`"
            ));
        }
        let touched_features = string_array(
            node.get("touched_feature_ids"),
            &format!("migration node {node_id} touched_feature_ids"),
        )?;
        if !touched_features.contains(owner_feature_id) {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` owner `{owner_feature_id}` is missing from touched_feature_ids"
            ));
        }
        for feature_id in &touched_features {
            if !feature_map.contains(&format!("`{feature_id}`")) {
                return Err(format!(
                    "OpenMinis UI migration node `{node_id}` references unknown touched feature `{feature_id}`"
                ));
            }
        }

        let source_paths = string_array(
            node.get("source_paths"),
            &format!("migration node {node_id} source_paths"),
        )?;
        if source_paths.is_empty()
            || source_paths
                .iter()
                .any(|path| !path.starts_with("src/ios/") || path.contains(".."))
        {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` source_paths must be OpenMinis-repository-relative src/ios paths"
            ));
        }
        let source_symbols = string_array(
            node.get("source_symbols"),
            &format!("migration node {node_id} source_symbols"),
        )?;
        if source_symbols.is_empty() {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` must name non-browser source symbols"
            ));
        }
        for symbol in source_symbols {
            let lower = symbol.to_ascii_lowercase();
            if OPENMINIS_UI_EXCLUDED_SOURCE_TOKENS
                .iter()
                .any(|token| lower.contains(token))
            {
                return Err(format!(
                    "OpenMinis UI migration node `{node_id}` includes excluded source symbol `{symbol}`"
                ));
            }
        }
        let source_semantic = required_string(
            node,
            "source_semantic",
            &format!("migration node {node_id}"),
        )?;
        if source_semantic.trim().is_empty() {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` has empty source_semantic"
            ));
        }
        let excluded_semantics = string_array(
            node.get("non_migrated_source_semantics"),
            &format!("migration node {node_id} non_migrated_source_semantics"),
        )?;
        for required in ["browser", "cookie", "browser_profile", "browser_takeover"] {
            if !excluded_semantics.contains(required) {
                return Err(format!(
                    "OpenMinis UI migration node `{node_id}` does not explicitly exclude `{required}`"
                ));
            }
        }

        verify_openminis_ui_map_documents(
            root,
            node_id,
            owner_feature_id,
            &touched_features,
            node,
        )?;
        for path in string_array(
            node.get("target_paths"),
            &format!("migration node {node_id} target_paths"),
        )? {
            existing_repository_path(
                root,
                &path,
                &format!("OpenMinis UI migration node `{node_id}` target path"),
            )?;
        }
        let verification_gates = string_array(
            node.get("verification_gates"),
            &format!("migration node {node_id} verification_gates"),
        )?;
        if !verification_gates.contains("openminis_ui_migration_manifest")
            || !verification_gates.contains("cargo_run_xtask_gates_check")
        {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` is not bound to the repository gate"
            ));
        }
        if !verification_gates.contains("webui_online_e2e") {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` is not bound to WebUI online E2E"
            ));
        }
        if node_id == "platform.android_bridge"
            && !verification_gates.contains("android_device_e2e")
        {
            return Err(
                "OpenMinis UI migration Android bridge is not bound to Android device E2E"
                    .to_owned(),
            );
        }
        let operation_id =
            required_string(node, "operation_id", &format!("migration node {node_id}"))?;
        if operation_id != "pending"
            && !operation_bindings.iter().any(|binding| {
                binding.get("operation_id").and_then(Value::as_str) == Some(operation_id)
            })
        {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` references unregistered operation `{operation_id}`"
            ));
        }
        if !matches!(
            status,
            "inventoried"
                | "blocked_resource_missing"
                | "blocked_owner_missing"
                | "blocked_protocol_missing"
        ) {
            verify_openminis_ui_node_operation_binding(
                node_id,
                node,
                &touched_features,
                &resource_map,
            )?;
        }

        if matches!(
            status,
            "source_bound" | "online_verified" | "legacy_retired" | "blocked_verification_missing"
        ) {
            verify_openminis_ui_target_bindings(root, node_id, node, operation_id)?;
        }
        if matches!(status, "online_verified" | "legacy_retired") {
            verify_openminis_ui_evidence(root, node_id, node, &verification_gates)?;
        }
        if status == "legacy_retired" {
            verify_openminis_ui_legacy_retirement(root, node_id, node, &verification_gates)?;
        }
    }
    if node_ids != expected_required {
        return Err(format!(
            "OpenMinis UI migration node set drift: expected {expected_required:?}, got {node_ids:?}"
        ));
    }
    verify_openminis_ui_manifest_phase(manifest_status, nodes)?;
    verify_openminis_ui_call_graph(root)?;
    verify_openminis_ui_pinned_source(root, source_repository, nodes)?;

    verify_openminis_ui_tree_topology(root, object, &node_ids)?;

    let human_tree = fs::read_to_string(root.join("docs/migrations/openminis-ui/ui-tree.md"))
        .map_err(|err| format!("read OpenMinis UI migration human tree: {err}"))?;
    let human_node_ids = inline_migration_node_ids(&human_tree);
    if human_node_ids != node_ids {
        return Err(format!(
            "OpenMinis UI migration human/machine node drift: manifest={node_ids:?}, human={human_node_ids:?}"
        ));
    }
    let dev_gates = fs::read_to_string(root.join("docs/architecture/dev-gates.md"))
        .map_err(|err| format!("read dev gates for OpenMinis UI migration gate: {err}"))?;
    require_contains(
        &dev_gates,
        "## OpenMinis UI Migration Manifest Gate",
        "docs/architecture/dev-gates.md",
    )?;
    Ok(())
}

mod binding;
mod call_graph;
mod declarations;
mod evidence;
mod lifecycle;
pub(crate) mod node_verifier;
mod pinned_source;
mod support;
mod topology;

use binding::*;
use call_graph::*;
use declarations::*;
use evidence::*;
use lifecycle::*;
use pinned_source::*;
use support::*;
use topology::*;

#[cfg(test)]
mod tests;
