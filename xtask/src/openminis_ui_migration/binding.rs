use super::*;

pub(super) fn verify_openminis_ui_node_operation_binding(
    node_id: &str,
    node: &serde_json::Map<String, Value>,
    touched_features: &BTreeSet<String>,
    resource_map: &Value,
) -> Result<(), String> {
    let operation_id = required_string(node, "operation_id", &format!("migration node {node_id}"))?;
    let bindings = resource_map
        .get("operation_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| "resource map missing operation_bindings".to_owned())?;
    let binding = bindings
        .iter()
        .find(|row| row.get("operation_id").and_then(Value::as_str) == Some(operation_id))
        .ok_or_else(|| format!("migration node `{node_id}` has no canonical operation binding"))?;
    let operation_owner = binding
        .get("owner_feature_id")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("operation `{operation_id}` missing owner_feature_id"))?;
    let source = binding
        .get("source_resource")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("operation `{operation_id}` missing source_resource"))?;
    let target = binding
        .get("target_resource")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("operation `{operation_id}` missing target_resource"))?;
    let node_sources = string_array(
        node.get("source_resources"),
        &format!("migration node {node_id} source_resources"),
    )?;
    if binding.get("binding_status").and_then(Value::as_str) != Some("bound")
        || node_sources != BTreeSet::from([source.to_owned()])
        || node.get("target_resource").and_then(Value::as_str) != Some(target)
        || !touched_features.contains(operation_owner)
    {
        return Err(format!(
            "migration node `{node_id}` resource/owner truth drifts from canonical operation `{operation_id}`"
        ));
    }
    let relation_allowed = resource_map
        .get("relation_rules")
        .and_then(Value::as_array)
        .is_some_and(|rules| {
            rules.iter().any(|rule| {
                rule.get("source_resource").and_then(Value::as_str) == Some(source)
                    && rule.get("target_resource").and_then(Value::as_str) == Some(target)
                    && rule.get("allowed_direct").and_then(Value::as_bool) == Some(true)
            })
        });
    if !relation_allowed {
        return Err(format!(
            "migration node `{node_id}` operation `{operation_id}` has no allowed direct resource relation"
        ));
    }
    if matches!(
        node.get("status").and_then(Value::as_str),
        Some(
            "contract_ready"
                | "implementation_in_progress"
                | "source_bound"
                | "online_verified"
                | "legacy_retired"
                | "blocked_verification_missing"
        )
    ) {
        verify_openminis_ui_operation_contract(node_id, node, binding)?;
    }
    Ok(())
}

fn verify_openminis_ui_operation_contract(
    node_id: &str,
    node: &serde_json::Map<String, Value>,
    binding: &Value,
) -> Result<(), String> {
    let contract = binding
        .get("ui_contract")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!(
                "migration node `{node_id}` operation has no canonical UI contract in resource truth"
            )
        })?;
    for field in ["projection_or_query", "generated_command", "surface_path"] {
        let expected = required_string(
            contract,
            field,
            &format!("migration node {node_id} canonical UI contract"),
        )?;
        if node.get(field).and_then(Value::as_str) != Some(expected) {
            return Err(format!(
                "migration node `{node_id}` `{field}` drifts from canonical UI contract `{expected}`"
            ));
        }
    }
    Ok(())
}

pub(super) fn verify_openminis_ui_target_bindings(
    root: &Path,
    node_id: &str,
    node: &serde_json::Map<String, Value>,
    operation_id: &str,
) -> Result<(), String> {
    let target_paths = string_array(
        node.get("target_paths"),
        &format!("migration node {node_id} target_paths"),
    )?;
    let target_symbols = string_array(
        node.get("target_symbols"),
        &format!("migration node {node_id} target_symbols"),
    )?;
    let mut target_files = Vec::new();
    for path in &target_paths {
        let target_root = existing_repository_path(
            root,
            path,
            &format!("migration node `{node_id}` target_path"),
        )?;
        collect_regular_files(&target_root, &mut target_files)?;
    }
    let mut declarations = BTreeMap::<String, Vec<DeclarationOccurrence>>::new();
    for path in target_files
        .iter()
        .filter(|path| is_declaration_source_path(path))
    {
        let source = fs::read_to_string(path)
            .map_err(|err| format!("read target source {}: {err}", path.display()))?;
        for declaration in declared_symbols(path, &source)? {
            declarations
                .entry(declaration.name.clone())
                .or_default()
                .push(declaration);
        }
    }
    let canonical_root = root
        .canonicalize()
        .map_err(|err| format!("canonicalize repository root {}: {err}", root.display()))?;
    let mut declaration_files = BTreeMap::new();
    for symbol in &target_symbols {
        let resolved = declarations.get(symbol).map(Vec::as_slice).unwrap_or(&[]);
        if resolved.len() != 1 {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` target symbol `{symbol}` must resolve as exactly one declaration in target_paths, found {}",
                resolved.len()
            ));
        }
        let file = Path::new(&resolved[0].file)
            .strip_prefix(&canonical_root)
            .map_err(|_| {
                format!(
                    "OpenMinis UI migration node `{node_id}` target symbol `{symbol}` declaration escaped repository: `{}`",
                    resolved[0].file
                )
            })?
            .to_string_lossy()
            .replace('\\', "/");
        declaration_files.insert(symbol.clone(), file);
    }

    let mut rows = Vec::new();
    for path in string_array(
        node.get("mainline_call_docs"),
        &format!("migration node {node_id} mainline_call_docs"),
    )? {
        let mainline_path = existing_repository_path(
            root,
            &path,
            &format!("migration node `{node_id}` mainline_call_doc"),
        )?;
        if !mainline_path.is_file() {
            return Err(format!(
                "migration node `{node_id}` mainline_call_doc is not a file: `{path}`"
            ));
        }
        let raw = fs::read_to_string(&mainline_path)
            .map_err(|err| format!("read migration node `{node_id}` mainline `{path}`: {err}"))?;
        let doc: Value = serde_json::from_str(&raw)
            .map_err(|err| format!("parse migration node `{node_id}` mainline `{path}`: {err}"))?;
        let call_table = doc
            .get("call_table")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                format!("migration node `{node_id}` mainline `{path}` has no call_table")
            })?;
        rows.extend(call_table.iter().cloned());
    }
    for symbol in &target_symbols {
        let declaration_file = declaration_files
            .get(symbol)
            .expect("unique declaration file recorded");
        let bound = rows.iter().any(|row| {
            let row_symbol = row
                .get("symbol_path")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let row_path = row
                .get("file_path")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let row_operation = row
                .get("resource_operation")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let binding_status = row
                .get("binding_status")
                .and_then(Value::as_str)
                .unwrap_or_default();
            split_binding_segments(row_symbol)
                .iter()
                .any(|segment| segment == symbol)
                && row_path == declaration_file
                && row_operation == operation_id
                && binding_status == "bound"
        });
        if !bound {
            return Err(format!(
                "OpenMinis UI migration node `{node_id}` target symbol `{symbol}` is not bound to its exact declaration file `{declaration_file}` and operation `{operation_id}` on a bound mainline row"
            ));
        }
    }
    verify_openminis_ui_resource_operation_bound(root, node_id, operation_id)
}

pub(super) fn verify_openminis_ui_resource_operation_bound(
    root: &Path,
    node_id: &str,
    operation_id: &str,
) -> Result<(), String> {
    let path = root.join("docs/resource-maps/core.json");
    let raw = fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    let resource_map: Value =
        serde_json::from_str(&raw).map_err(|err| format!("parse {}: {err}", path.display()))?;
    let bindings = resource_map
        .get("operation_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| "resource map missing operation_bindings".to_owned())?;
    let binding = bindings
        .iter()
        .find(|binding| binding.get("operation_id").and_then(Value::as_str) == Some(operation_id))
        .ok_or_else(|| {
            format!(
                "OpenMinis UI migration node `{node_id}` references unregistered operation `{operation_id}`"
            )
        })?;
    if binding.get("binding_status").and_then(Value::as_str) != Some("bound") {
        return Err(format!(
            "OpenMinis UI migration node `{node_id}` operation `{operation_id}` is not bound in the resource map"
        ));
    }
    Ok(())
}
