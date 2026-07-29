use super::super::*;
use super::common::*;

#[test]
fn openminis_ui_migration_manifest_accepts_current_design_baseline() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    verify_openminis_ui_migration_manifest(&root)
        .expect("current OpenMinis UI migration registry should pass");
}

#[test]
fn openminis_ui_migration_manifest_rejects_unbound_advanced_status() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let raw = fs::read_to_string(root.join("docs/migrations/openminis-ui/ui-tree.manifest.json"))
        .expect("read migration manifest");
    let mut manifest: Value = serde_json::from_str(&raw).expect("parse migration manifest");
    manifest["nodes"][0]["status"] = Value::String("source_bound".to_owned());

    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("unbound advanced state must fail");
    assert!(
        err.contains("without mapped resource and operation truth"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_manifest_rejects_missing_machine_node() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let raw = fs::read_to_string(root.join("docs/migrations/openminis-ui/ui-tree.manifest.json"))
        .expect("read migration manifest");
    let mut manifest: Value = serde_json::from_str(&raw).expect("parse migration manifest");
    manifest["nodes"]
        .as_array_mut()
        .expect("nodes array")
        .retain(|node| node["node_id"] != "tools.activity");

    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("missing machine node must fail");
    assert!(err.contains("node set drift"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_browser_source_symbol() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let raw = fs::read_to_string(root.join("docs/migrations/openminis-ui/ui-tree.manifest.json"))
        .expect("read migration manifest");
    let mut manifest: Value = serde_json::from_str(&raw).expect("parse migration manifest");
    manifest["nodes"][0]["source_symbols"][0] = Value::String("BrowserSheetView".to_owned());

    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("browser source symbol must fail");
    assert!(err.contains("excluded source symbol"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_unknown_status() {
    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    manifest["nodes"][0]["status"] = Value::String("blocked_typo".to_owned());

    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("unknown lifecycle status must fail");
    assert!(err.contains("unknown status `blocked_typo`"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_external_or_misidentified_map_paths() {
    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    manifest["nodes"][0]["function_map_docs"] = serde_json::json!(["/etc/passwd"]);

    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("absolute host map path must fail in the inventoried state");
    assert!(err.contains("repository-relative"), "{err}");

    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    manifest["nodes"][0]["mainline_call_docs"] =
        serde_json::json!(["docs/mainline-calls/ui.protocol.json"]);
    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("cross-kind feature-id set drift must fail");
    assert!(err.contains("map feature-id drift"), "{err}");

    let fixture_root = test_repo_root("misidentified-map");
    fs::create_dir_all(fixture_root.join("docs/function-maps")).expect("function map dir");
    fs::write(
        fixture_root.join("docs/function-maps/app.webui-smoke.md"),
        "# Function Map: `wrong.feature`\n\n- feature_id: `wrong.feature`\n",
    )
    .expect("forged function map");
    let err = verify_openminis_ui_map_path(
        &fixture_root,
        "foundation.root",
        "app.webui-smoke",
        MapDocumentKind::Function,
        "docs/function-maps/app.webui-smoke.md",
    )
    .expect_err("path/content identity mismatch must fail");
    assert!(err.contains("self identity"), "{err}");
}

#[test]
fn openminis_ui_migration_rejects_operation_resource_drift_and_online_only_completion() {
    let (root, manifest) = openminis_ui_migration_test_manifest();
    let resource_map: Value = serde_json::from_str(
        &fs::read_to_string(root.join("docs/resource-maps/core.json")).expect("resource map"),
    )
    .expect("parse resource map");
    let mut node = manifest["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["node_id"] == "home.dashboard")
        .expect("home node")
        .clone();
    node["source_resources"] = serde_json::json!(["invented_resource"]);
    let touched = string_array(node.get("touched_feature_ids"), "touched").expect("touched");
    let err = verify_openminis_ui_node_operation_binding(
        "home.dashboard",
        node.as_object().expect("node"),
        &touched,
        &resource_map,
    )
    .expect_err("invented operation endpoint must fail");
    assert!(err.contains("drifts from canonical operation"), "{err}");

    let nodes = manifest["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| {
            let mut node = node.clone();
            node["status"] = Value::String("online_verified".to_owned());
            node
        })
        .collect::<Vec<_>>();
    let err = verify_openminis_ui_manifest_phase("migration_complete", &nodes)
        .expect_err("online-only nodes cannot complete retirement");
    assert!(
        err.contains("requires every included node legacy_retired"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_manifest_rejects_human_machine_edge_drift() {
    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    manifest["edges"][0]["semantic"] = Value::String("fabricated semantic".to_owned());

    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("machine edge semantic drift must fail");
    assert!(err.contains("human forward-edge registry drift"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_forged_target_symbol() {
    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    bind_first_migration_node_for_test(&mut manifest, "__missing_target_symbol__");
    manifest["nodes"][0]["status"] = Value::String("source_bound".to_owned());

    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("nonexistent target symbol must fail");
    assert!(
        err.contains("must resolve as exactly one declaration"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_manifest_rejects_unstructured_online_evidence() {
    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    bind_first_migration_node_for_test(&mut manifest, "maybeNotifyAndroidTurnFinished");
    manifest["nodes"][0]["status"] = Value::String("online_verified".to_owned());
    manifest["nodes"][0]["evidence"] = Value::Array(vec![Value::String("passed".to_owned())]);

    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("unstructured online evidence must fail");
    assert!(err.contains("evidence must contain objects"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_accepts_owner_mapped_without_target_symbols() {
    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    manifest["status"] = Value::String("migration_in_progress".to_owned());
    let node = &mut manifest["nodes"][0];
    node["status"] = Value::String("owner_mapped".to_owned());
    node["operation_id"] =
        Value::String("ui_projection.post_android_turn_finished_notification".to_owned());
    node["source_resources"] = serde_json::json!(["ui_projection"]);
    node["target_resource"] = Value::String("android_notification".to_owned());
    node["target_symbols"] = serde_json::json!([]);

    verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect("owner_mapped must not require target symbols");
}

#[test]
fn openminis_ui_migration_manifest_rejects_contract_ready_pending_protocol() {
    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    manifest["status"] = Value::String("migration_in_progress".to_owned());
    let node = &mut manifest["nodes"][0];
    node["status"] = Value::String("contract_ready".to_owned());
    node["operation_id"] =
        Value::String("ui_projection.post_android_turn_finished_notification".to_owned());
    node["source_resources"] = serde_json::json!(["ui_projection"]);
    node["projection_or_query"] = Value::String("pending".to_owned());

    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("contract_ready with pending protocol must fail");
    assert!(err.contains("`projection_or_query` is pending"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_source_bound_without_target_symbols() {
    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    bind_first_migration_node_for_test(&mut manifest, "maybeNotifyAndroidTurnFinished");
    manifest["nodes"][0]["status"] = Value::String("source_bound".to_owned());
    manifest["nodes"][0]["target_symbols"] = serde_json::json!([]);

    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("source_bound without target symbols must fail");
    assert!(err.contains("without target_symbols"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_blocked_state_without_pending_boundary() {
    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    let node = manifest["nodes"]
        .as_array_mut()
        .expect("nodes")
        .iter_mut()
        .find(|node| node["status"] == "blocked_resource_missing")
        .expect("blocked resource node");
    node["operation_id"] =
        Value::String("ui_projection.post_android_turn_finished_notification".to_owned());
    node["source_resources"] = serde_json::json!(["ui_projection"]);

    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("blocked state cannot masquerade as implemented");
    assert!(
        err.contains("must retain a pending resource/owner boundary"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_manifest_accepts_exact_source_bound_binding() {
    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    bind_first_migration_node_for_test(&mut manifest, "maybeNotifyAndroidTurnFinished");
    manifest["nodes"][0]["status"] = Value::String("source_bound".to_owned());

    verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect("exact source-bound symbol/row/operation should pass");
}

#[test]
fn openminis_ui_migration_manifest_rejects_promoted_route_edge_drift() {
    for (name, route_edge_ids) in [
        ("missing", serde_json::json!([])),
        (
            "unrelated",
            serde_json::json!(["ui_tree.tools.registry.to.tools.detail"]),
        ),
    ] {
        let (root, mut manifest) = openminis_ui_migration_test_manifest();
        bind_first_migration_node_for_test(&mut manifest, "maybeNotifyAndroidTurnFinished");
        manifest["nodes"][0]["status"] = Value::String("contract_ready".to_owned());
        manifest["nodes"][0]["route_edge_ids"] = route_edge_ids;

        let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
            .expect_err("promoted node route-edge drift must fail");
        assert!(err.contains("route_edge_ids drift"), "{name}: {err}");
    }
}

#[test]
fn openminis_ui_migration_manifest_rejects_operation_owned_ui_contract_drift() {
    for (field, fabricated) in [
        ("projection_or_query", "InventedProjection"),
        ("generated_command", "InventedCommand"),
        (
            "surface_path",
            "apps/freehand-server/assets/webui/invented.js",
        ),
    ] {
        let (root, mut manifest) = openminis_ui_migration_test_manifest();
        bind_first_migration_node_for_test(&mut manifest, "maybeNotifyAndroidTurnFinished");
        manifest["nodes"][0]["status"] = Value::String("contract_ready".to_owned());
        manifest["nodes"][0][field] = Value::String(fabricated.to_owned());

        let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
            .expect_err("fabricated UI contract field must fail");
        assert!(err.contains("canonical UI contract"), "{field}: {err}");
    }
}
