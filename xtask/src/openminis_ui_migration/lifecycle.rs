use super::*;

pub(super) fn verify_openminis_ui_lifecycle_fields(
    node_id: &str,
    status: &str,
    node: &serde_json::Map<String, Value>,
) -> Result<(), String> {
    let context = format!("migration node {node_id}");
    let operation_id = required_string(node, "operation_id", &context)?;
    let projection_or_query = required_string(node, "projection_or_query", &context)?;
    let generated_command = required_string(node, "generated_command", &context)?;
    let surface_path = required_string(node, "surface_path", &context)?;
    let source_resources = string_array(
        node.get("source_resources"),
        &format!("{context} source_resources"),
    )?;
    let target_resource = required_string(node, "target_resource", &context)?;
    let target_paths = string_array(node.get("target_paths"), &format!("{context} target_paths"))?;
    let target_symbols = string_array(
        node.get("target_symbols"),
        &format!("{context} target_symbols"),
    )?;
    let pending_source_resource = source_resources
        .iter()
        .any(|resource| resource == "pending" || resource.ends_with("_pending"));
    let require_owner_mapping = || -> Result<(), String> {
        if operation_id == "pending"
            || source_resources.is_empty()
            || pending_source_resource
            || target_resource == "pending"
            || target_resource.ends_with("_pending")
        {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` cannot be `{status}` without mapped resource and operation truth"
            ));
        }
        Ok(())
    };
    let require_contract = || -> Result<(), String> {
        require_owner_mapping()?;
        for (field, value) in [
            ("projection_or_query", projection_or_query),
            ("generated_command", generated_command),
            ("surface_path", surface_path),
        ] {
            if value == "pending" {
                return Err(format!(
                    "OpenMinis UI migration node `{node_id}` cannot be `{status}` while `{field}` is pending"
                ));
            }
        }
        if target_paths.is_empty() {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` cannot be `{status}` without target_paths"
            ));
        }
        Ok(())
    };

    match status {
        "inventoried" => Ok(()),
        "owner_mapped" => require_owner_mapping(),
        "contract_ready" | "implementation_in_progress" => require_contract(),
        "source_bound" | "online_verified" | "legacy_retired" => {
            require_contract()?;
            if target_symbols.is_empty() {
                return Err(format!(
                    "OpenMinis UI migration node `{node_id}` cannot be `{status}` without target_symbols"
                ));
            }
            Ok(())
        }
        "blocked_resource_missing" | "blocked_owner_missing" => {
            if operation_id != "pending" || !pending_source_resource {
                return Err(format!(
                    "OpenMinis UI migration node `{node_id}` status `{status}` must retain a pending resource/owner boundary"
                ));
            }
            Ok(())
        }
        "blocked_protocol_missing" => {
            if operation_id != "pending" || projection_or_query != "pending" {
                return Err(format!(
                    "OpenMinis UI migration node `{node_id}` status `blocked_protocol_missing` must retain pending operation and projection truth"
                ));
            }
            Ok(())
        }
        "blocked_verification_missing" => {
            require_contract()?;
            if target_symbols.is_empty() {
                return Err(format!(
                    "OpenMinis UI migration node `{node_id}` status `blocked_verification_missing` requires source-bound target symbols"
                ));
            }
            Ok(())
        }
        _ => Err(format!(
            "OpenMinis UI migration node `{node_id}` has unsupported lifecycle status `{status}`"
        )),
    }
}

pub(super) fn verify_openminis_ui_manifest_phase(
    manifest_status: &str,
    nodes: &[Value],
) -> Result<(), String> {
    let statuses = nodes
        .iter()
        .filter_map(|node| node.get("status").and_then(Value::as_str))
        .collect::<Vec<_>>();
    let baseline = |status: &&str| status == &"inventoried" || status.starts_with("blocked_");
    match manifest_status {
        "design_baseline" if statuses.iter().all(baseline) => Ok(()),
        "design_baseline" => Err(
            "OpenMinis UI migration manifest status `design_baseline` cannot contain promoted lifecycle nodes"
                .to_owned(),
        ),
        "migration_in_progress"
            if statuses.iter().any(|status| !baseline(status))
                && !statuses.iter().all(|status| *status == "legacy_retired") =>
        {
            Ok(())
        }
        "migration_in_progress" => Err(
            "OpenMinis UI migration manifest status `migration_in_progress` requires promoted but not fully terminal nodes"
                .to_owned(),
        ),
        "migration_complete" if statuses.iter().all(|status| *status == "legacy_retired") => Ok(()),
        "migration_complete" => Err(
            "OpenMinis UI migration manifest status `migration_complete` requires every included node legacy_retired"
                .to_owned(),
        ),
        _ => Err(format!(
            "OpenMinis UI migration manifest has unsupported status `{manifest_status}`"
        )),
    }
}
