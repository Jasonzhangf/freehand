use super::*;

struct LegacyIdentities<'a> {
    paths: &'a BTreeSet<String>,
    symbols: &'a BTreeSet<String>,
    import_tokens: &'a BTreeSet<String>,
    callers: &'a BTreeSet<String>,
}

fn verify_owner_bound_legacy_scan_roots(
    root: &Path,
    node_id: &str,
    node: &serde_json::Map<String, Value>,
    declared_scan_paths: &BTreeSet<String>,
    identities: &LegacyIdentities<'_>,
) -> Result<Vec<PathBuf>, String> {
    let owner_feature_id = required_string(
        node,
        "owner_feature_id",
        &format!("migration node {node_id}"),
    )?;
    let mainline_call_docs = string_array(
        node.get("mainline_call_docs"),
        &format!("migration node {node_id} mainline_call_docs"),
    )?;
    let mut matches = Vec::new();
    for mainline_path in mainline_call_docs {
        let path = existing_repository_path(
            root,
            &mainline_path,
            &format!("migration node `{node_id}` owner mainline"),
        )?;
        if !path.is_file() {
            return Err(format!(
                "migration node `{node_id}` owner mainline is not a file: `{mainline_path}`"
            ));
        }
        let raw = fs::read_to_string(&path)
            .map_err(|err| format!("read owner mainline `{mainline_path}`: {err}"))?;
        let doc: Value = serde_json::from_str(&raw)
            .map_err(|err| format!("parse owner mainline `{mainline_path}`: {err}"))?;
        let doc = doc
            .as_object()
            .ok_or_else(|| format!("owner mainline `{mainline_path}` must be an object"))?;
        let doc_feature_id = required_string(doc, "feature_id", "owner mainline")?;
        let Some(rows) = doc.get("legacy_scan_roots") else {
            continue;
        };
        let rows = rows.as_array().ok_or_else(|| {
            format!("owner mainline `{mainline_path}` legacy_scan_roots must be an array")
        })?;
        for row in rows {
            let row = row.as_object().ok_or_else(|| {
                format!("owner mainline `{mainline_path}` legacy_scan_roots must contain objects")
            })?;
            let registered_node_id = required_string(
                row,
                "node_id",
                &format!("owner mainline {mainline_path} legacy_scan_roots"),
            )?;
            if registered_node_id != node_id {
                continue;
            }
            let registered_mainline_path = required_string(
                doc,
                "mainline_call_doc",
                &format!("owner mainline {mainline_path}"),
            )?;
            let relative_mainline_path = repository_relative_path(
                &mainline_path,
                &format!("migration node `{node_id}` owner mainline"),
            )?;
            if registered_mainline_path != mainline_path
                || !relative_mainline_path.starts_with("docs/mainline-calls")
                || relative_mainline_path
                    .extension()
                    .and_then(|value| value.to_str())
                    != Some("json")
            {
                return Err(format!(
                    "migration node `{node_id}` legacy_scan_roots row must live in its canonical machine mainline: listed `{mainline_path}`, self-registered `{registered_mainline_path}`"
                ));
            }
            let registered_owner = required_string(
                row,
                "owner_feature_id",
                &format!("owner mainline {mainline_path} legacy_scan_roots node {node_id}"),
            )?;
            let registered_scan_paths = string_array(
                row.get("scan_paths"),
                &format!(
                    "owner mainline {mainline_path} legacy_scan_roots node {node_id} scan_paths"
                ),
            )?;
            let registered_removed_paths = string_array(
                row.get("removed_paths"),
                &format!(
                    "owner mainline {mainline_path} legacy_scan_roots node {node_id} removed_paths"
                ),
            )?;
            let registered_removed_symbols = string_array(
                row.get("removed_symbols"),
                &format!(
                    "owner mainline {mainline_path} legacy_scan_roots node {node_id} removed_symbols"
                ),
            )?;
            let registered_removed_import_tokens = string_array(
                row.get("removed_import_tokens"),
                &format!(
                    "owner mainline {mainline_path} legacy_scan_roots node {node_id} removed_import_tokens"
                ),
            )?;
            let registered_removed_callers = string_array(
                row.get("removed_callers"),
                &format!(
                    "owner mainline {mainline_path} legacy_scan_roots node {node_id} removed_callers"
                ),
            )?;
            matches.push((
                mainline_path.clone(),
                doc_feature_id.to_owned(),
                registered_owner.to_owned(),
                registered_scan_paths,
                registered_removed_paths,
                registered_removed_symbols,
                registered_removed_import_tokens,
                registered_removed_callers,
            ));
        }
    }
    if matches.len() != 1 {
        return Err(format!(
            "migration node `{node_id}` requires exactly one owner-bound legacy_scan_roots row, found {}",
            matches.len()
        ));
    }
    let (
        mainline_path,
        doc_feature_id,
        registered_owner,
        registered_scan_paths,
        registered_removed_paths,
        registered_removed_symbols,
        registered_removed_import_tokens,
        registered_removed_callers,
    ) = matches.pop().expect("one checked match");
    if doc_feature_id != owner_feature_id || registered_owner != owner_feature_id {
        return Err(format!(
            "migration node `{node_id}` legacy_scan_roots owner drift: node owner `{owner_feature_id}`, mainline `{mainline_path}` feature `{doc_feature_id}`, registry owner `{registered_owner}`"
        ));
    }
    if &registered_scan_paths != declared_scan_paths {
        return Err(format!(
            "migration node `{node_id}` legacy scan_paths drift from owner registry: declared={declared_scan_paths:?}, registered={registered_scan_paths:?}"
        ));
    }
    for (field, declared, registered) in [
        ("removed_paths", identities.paths, &registered_removed_paths),
        (
            "removed_symbols",
            identities.symbols,
            &registered_removed_symbols,
        ),
        (
            "removed_import_tokens",
            identities.import_tokens,
            &registered_removed_import_tokens,
        ),
        (
            "removed_callers",
            identities.callers,
            &registered_removed_callers,
        ),
    ] {
        if declared != registered {
            return Err(format!(
                "migration node `{node_id}` legacy {field} drift from owner registry: declared={declared:?}, registered={registered:?}"
            ));
        }
    }

    let mut canonical_scan_roots = Vec::new();
    for scan_path in declared_scan_paths {
        let scan_root = existing_repository_path(
            root,
            scan_path,
            &format!("migration node `{node_id}` owner-bound legacy scan_path"),
        )?;
        if !scan_root.is_dir() {
            return Err(format!(
                "migration node `{node_id}` owner-bound legacy scan_path must be a directory: `{scan_path}`"
            ));
        }
        canonical_scan_roots.push(scan_root);
    }

    let target_paths = string_array(
        node.get("target_paths"),
        &format!("migration node {node_id} target_paths"),
    )?;
    for target_path in target_paths {
        let target = existing_repository_path(
            root,
            &target_path,
            &format!("migration node `{node_id}` bound target_path"),
        )?;
        if !canonical_scan_roots
            .iter()
            .any(|scan_root| target.starts_with(scan_root))
        {
            return Err(format!(
                "migration node `{node_id}` owner-bound legacy scan roots do not cover bound target_path `{target_path}`"
            ));
        }
    }

    for removed_path in identities.paths {
        let removed = repository_relative_path(
            removed_path,
            &format!("migration node `{node_id}` legacy retirement removed_path"),
        )?;
        if !declared_scan_paths.iter().any(|scan_path| {
            let scan_path = Path::new(scan_path);
            removed == scan_path || removed.starts_with(scan_path)
        }) {
            return Err(format!(
                "migration node `{node_id}` owner-bound legacy scan roots do not cover removed_path `{removed_path}`"
            ));
        }
    }

    Ok(canonical_scan_roots)
}

pub(super) fn check_openminis_ui_legacy_retirement(
    root: &Path,
    node_id: &str,
    node: &serde_json::Map<String, Value>,
    verification_gates: &BTreeSet<String>,
) -> Result<(), String> {
    let retirement = node
        .get("legacy_retirement")
        .and_then(Value::as_object)
        .ok_or_else(|| {
            format!("migration node `{node_id}` legacy_retired status requires legacy_retirement")
        })?;
    if retirement.get("required").and_then(Value::as_bool) != Some(true) {
        return Err(format!(
            "migration node `{node_id}` legacy_retirement.required must be true"
        ));
    }
    let scan_paths = string_array(
        retirement.get("scan_paths"),
        &format!("migration node {node_id} legacy_retirement.scan_paths"),
    )?;
    if scan_paths.is_empty() {
        return Err(format!(
            "migration node `{node_id}` legacy retirement requires scan_paths"
        ));
    }
    let removed_paths = string_array(
        retirement.get("removed_paths"),
        &format!("migration node {node_id} legacy_retirement.removed_paths"),
    )?;
    let removed_symbols = string_array(
        retirement.get("removed_symbols"),
        &format!("migration node {node_id} legacy_retirement.removed_symbols"),
    )?;
    let removed_import_tokens = string_array(
        retirement.get("removed_import_tokens"),
        &format!("migration node {node_id} legacy_retirement.removed_import_tokens"),
    )?;
    let removed_callers = string_array(
        retirement.get("removed_callers"),
        &format!("migration node {node_id} legacy_retirement.removed_callers"),
    )?;
    if removed_paths.is_empty()
        && removed_symbols.is_empty()
        && removed_import_tokens.is_empty()
        && removed_callers.is_empty()
    {
        return Err(format!(
            "migration node `{node_id}` legacy retirement must declare at least one removed identity"
        ));
    }
    for path in &scan_paths {
        repository_relative_path(
            path,
            &format!("migration node `{node_id}` legacy retirement scan_path"),
        )?;
    }
    let identities = LegacyIdentities {
        paths: &removed_paths,
        symbols: &removed_symbols,
        import_tokens: &removed_import_tokens,
        callers: &removed_callers,
    };
    let scan_roots =
        verify_owner_bound_legacy_scan_roots(root, node_id, node, &scan_paths, &identities)?;
    for path in &removed_paths {
        let relative = repository_relative_path(
            path,
            &format!("migration node `{node_id}` legacy retirement removed_path"),
        )?;
        match fs::symlink_metadata(root.join(relative)) {
            Ok(_) => {
                return Err(format!(
                    "migration node `{node_id}` legacy path still exists: `{path}`"
                ));
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                return Err(format!(
                    "migration node `{node_id}` cannot inspect removed legacy path `{path}`: {err}"
                ));
            }
        }
    }
    let mut scan_files = Vec::new();
    for scan_root in scan_roots {
        collect_regular_files(&scan_root, &mut scan_files)?;
    }
    let mut scan_bytes = Vec::new();
    for file in scan_files {
        let bytes = fs::read(&file)
            .map_err(|err| format!("read legacy scan file {}: {err}", file.display()))?;
        scan_bytes.extend_from_slice(&bytes);
        scan_bytes.push(b'\n');
    }
    for (kind, tokens) in [
        ("symbol", &removed_symbols),
        ("import token", &removed_import_tokens),
        ("caller", &removed_callers),
    ] {
        for token in tokens {
            if scan_bytes
                .windows(token.len())
                .any(|window| window == token.as_bytes())
            {
                return Err(format!(
                    "migration node `{node_id}` legacy {kind} still resolves under scan_paths: `{token}`"
                ));
            }
        }
    }

    let no_touch_gate = required_string(
        retirement,
        "online_no_touch_gate_id",
        &format!("migration node {node_id} legacy_retirement"),
    )?;
    if no_touch_gate != "openminis_ui_legacy_online_no_touch"
        || !verification_gates.contains(no_touch_gate)
    {
        return Err(format!(
            "migration node `{node_id}` legacy retirement requires verification gate `openminis_ui_legacy_online_no_touch`"
        ));
    }
    let evidence = node
        .get("evidence")
        .and_then(Value::as_array)
        .ok_or_else(|| format!("migration node `{node_id}` evidence must be an array"))?;
    let record = evidence
        .iter()
        .filter_map(Value::as_object)
        .find(|record| record.get("gate_id").and_then(Value::as_str) == Some(no_touch_gate))
        .ok_or_else(|| {
            format!("migration node `{node_id}` legacy retirement lacks online no-touch evidence")
        })?;
    let artifact_path = required_string(
        record,
        "artifact_path",
        &format!("migration node {node_id} legacy no-touch evidence"),
    )?;
    let artifact =
        read_openminis_ui_evidence_artifact(root, node_id, no_touch_gate, artifact_path)?;
    if artifact.get("legacy_touched").and_then(Value::as_bool) != Some(false) {
        return Err(format!(
            "migration node `{node_id}` legacy no-touch artifact must prove legacy_touched=false"
        ));
    }
    Ok(())
}
