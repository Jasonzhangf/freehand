use super::*;

pub(crate) fn verify_openminis_ui_migration_node(
    root: &Path,
    requested_node_id: &str,
) -> Result<(), String> {
    let manifest_path = root.join("docs/migrations/openminis-ui/ui-tree.manifest.json");
    let raw = fs::read_to_string(&manifest_path)
        .map_err(|err| format!("{}: {err}", manifest_path.display()))?;
    let mut manifest: Value =
        serde_json::from_str(&raw).map_err(|err| format!("{}: {err}", manifest_path.display()))?;
    let object = manifest
        .as_object_mut()
        .ok_or_else(|| "OpenMinis UI migration manifest must be an object".to_owned())?;
    object.insert(
        "status".to_owned(),
        Value::String("migration_in_progress".to_owned()),
    );
    let nodes = object
        .get_mut("nodes")
        .and_then(Value::as_array_mut)
        .ok_or_else(|| "OpenMinis UI migration manifest missing nodes".to_owned())?;
    let mut requested_found = false;
    for node in nodes {
        let node = node
            .as_object_mut()
            .ok_or_else(|| "OpenMinis UI migration nodes must contain only objects".to_owned())?;
        let node_id = required_string(node, "node_id", "OpenMinis UI migration node")?;
        if node_id == requested_node_id {
            requested_found = true;
            let status = required_string(node, "status", &format!("migration node {node_id}"))?;
            if !matches!(
                status,
                "source_bound" | "online_verified" | "legacy_retired"
            ) {
                return Err(format!(
                    "migration node `{node_id}` must be source_bound before node verification, got `{status}`"
                ));
            }
        }
        if matches!(
            node.get("status").and_then(Value::as_str),
            Some("online_verified" | "legacy_retired")
        ) {
            node.insert(
                "status".to_owned(),
                Value::String("source_bound".to_owned()),
            );
            node.remove("evidence");
            node.remove("legacy_retirement");
        }
    }
    if !requested_found {
        return Err(format!(
            "OpenMinis UI migration manifest has no node `{requested_node_id}`"
        ));
    }
    verify_openminis_ui_migration_manifest_value(root, &manifest)
}
