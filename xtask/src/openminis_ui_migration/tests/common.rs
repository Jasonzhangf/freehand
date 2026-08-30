use super::super::*;
use ed25519_dalek::{Signer, SigningKey};
use sha2::{Digest, Sha256};
use std::ops::Deref;

pub(super) struct TestRepoRoot(tempfile::TempDir);

impl Deref for TestRepoRoot {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        self.0.path()
    }
}

pub(super) fn test_repo_root(name: &str) -> TestRepoRoot {
    TestRepoRoot(
        tempfile::Builder::new()
            .prefix(&format!("freehand-openminis-xtask-{name}-"))
            .tempdir()
            .expect("create temp repo"),
    )
}

pub(super) fn openminis_ui_migration_test_manifest() -> (PathBuf, Value) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let raw = fs::read_to_string(root.join("docs/migrations/openminis-ui/ui-tree.manifest.json"))
        .expect("read migration manifest");
    let manifest = serde_json::from_str(&raw).expect("parse migration manifest");
    (root, manifest)
}

pub(super) fn bind_first_migration_node_for_test(manifest: &mut Value, target_symbol: &str) {
    manifest["status"] = Value::String("migration_in_progress".to_owned());
    let node_id = manifest["nodes"][0]["node_id"]
        .as_str()
        .expect("first node id")
        .to_owned();
    let route_edge_ids = manifest["edges"]
        .as_array()
        .expect("manifest edges")
        .iter()
        .filter(|edge| edge["from_node_id"] == node_id || edge["to_node_id"] == node_id)
        .map(|edge| edge["edge_id"].clone())
        .collect::<Vec<_>>();
    let node = &mut manifest["nodes"][0];
    node["operation_id"] =
        Value::String("ui_projection.post_android_turn_finished_notification".to_owned());
    node["source_resources"] = serde_json::json!(["ui_projection"]);
    node["target_resource"] = Value::String("android_notification".to_owned());
    node["projection_or_query"] = Value::String("UiTurnProjection".to_owned());
    node["generated_command"] = Value::String("none".to_owned());
    node["surface_path"] =
        Value::String("apps/freehand-server/assets/webui/legacy-monolith.js".to_owned());
    node["target_paths"] =
        serde_json::json!(["apps/freehand-server/assets/webui/legacy-monolith.js"]);
    node["target_symbols"] = serde_json::json!([target_symbol]);
    node["route_edge_ids"] = Value::Array(route_edge_ids);
}

pub(super) fn run_test_git(repository: &Path, args: &[&str]) {
    let output = isolated_git_command(repository)
        .args(args)
        .output()
        .expect("run test git");
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn commit_test_repository_baseline(root: &Path) -> (String, String) {
    run_test_git(root, &["init"]);
    run_test_git(root, &["config", "user.email", "xtask@example.invalid"]);
    run_test_git(root, &["config", "user.name", "xtask"]);
    run_test_git(root, &["add", "."]);
    run_test_git(root, &["commit", "--allow-empty", "-m", "baseline"]);
    let revision = |spec: &str| {
        let output = isolated_git_command(root)
            .args(["rev-parse", spec])
            .output()
            .expect("read baseline revision");
        assert!(output.status.success(), "read baseline revision {spec}");
        String::from_utf8(output.stdout)
            .expect("revision is UTF-8")
            .trim()
            .to_owned()
    };
    (revision("HEAD"), revision("HEAD^{tree}"))
}

pub(super) fn manifest_node_ids(manifest: &Value) -> BTreeSet<String> {
    manifest["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .map(|node| node["node_id"].as_str().expect("node id").to_owned())
        .collect()
}

pub(super) fn write_openminis_binding_fixture(
    name: &str,
    symbol_mode: &str,
    row_binding_status: &str,
    operation_binding_status: &str,
) -> (TestRepoRoot, Value) {
    let root = test_repo_root(&format!("openminis-binding-{name}"));
    fs::create_dir_all(root.join("src")).expect("create src");
    fs::create_dir_all(root.join("docs/mainline-calls")).expect("create mainlines");
    fs::create_dir_all(root.join("docs/resource-maps")).expect("create resource maps");
    fs::write(root.join("src/target.js"), "function exactSymbol() {}\n").expect("write target");
    let row_symbol = if symbol_mode == "exact" {
        "exactSymbol"
    } else {
        "exactSymbolSuffix"
    };
    let mainline = serde_json::json!({
        "call_table": [{
            "symbol_path": row_symbol,
            "file_path": "src/target.js",
            "resource_operation": "ui_projection.render",
            "binding_status": row_binding_status
        }]
    });
    fs::write(
        root.join("docs/mainline-calls/test.json"),
        serde_json::to_vec(&mainline).expect("encode mainline"),
    )
    .expect("write mainline");
    let resource_map = serde_json::json!({
        "operation_bindings": [{
            "operation_id": "ui_projection.render",
            "binding_status": operation_binding_status
        }]
    });
    fs::write(
        root.join("docs/resource-maps/core.json"),
        serde_json::to_vec(&resource_map).expect("encode resource map"),
    )
    .expect("write resource map");
    let node = serde_json::json!({
        "target_paths": ["src/target.js"],
        "target_symbols": ["exactSymbol"],
        "mainline_call_docs": ["docs/mainline-calls/test.json"]
    });
    (root, node)
}

pub(super) fn write_openminis_evidence_fixture(
    name: &str,
) -> (TestRepoRoot, Value, BTreeSet<String>) {
    let root = test_repo_root(&format!("openminis-evidence-{name}"));
    fs::create_dir_all(root.join("docs/migrations/openminis-ui/evidence"))
        .expect("create evidence dir");
    fs::write(
        root.join("docs/migrations/openminis-ui/ui-tree.manifest.json"),
        serde_json::to_vec(&serde_json::json!({
            "status": "migration_in_progress",
            "nodes": [{
                "node_id": "foundation.root",
                "status": "source_bound",
                "operation_id": "ui_projection.render_foundation",
                "evidence": []
            }]
        }))
        .expect("encode baseline lifecycle manifest"),
    )
    .expect("write baseline lifecycle manifest");
    let (repository_commit, repository_tree) = commit_test_repository_baseline(&root);
    let gates = [
        "openminis_ui_migration_manifest".to_owned(),
        "webui_online_e2e".to_owned(),
    ]
    .into_iter()
    .collect::<BTreeSet<_>>();
    let mut records = Vec::new();
    for gate_id in &gates {
        let (command, proof_kind, verifier_id, assertions) = if gate_id == "webui_online_e2e" {
            (
                "make verify-webui-online",
                "webui_online",
                "freehand.webui_online",
                serde_json::json!({
                    "daemon_hosted": true,
                    "owner_truth_verified": true,
                    "dom_assertions_passed": true
                }),
            )
        } else {
            (
                "cargo run -p xtask -- openminis-ui verify-node foundation.root",
                "node_repository_gate",
                "xtask.openminis-ui-node",
                serde_json::json!({"node_source_gates_passed": true}),
            )
        };
        let artifact_path = format!("docs/migrations/openminis-ui/evidence/{gate_id}.json");
        let report_path = format!("docs/migrations/openminis-ui/evidence/{gate_id}-report.json");
        let mut report = serde_json::json!({
            "schema_version": "freehand.verifier-report.v1",
            "verifier_id": verifier_id,
            "node_id": "foundation.root",
            "migration_unit_id": "ui_migration.foundation.root",
            "command": command,
            "online_run_id": "online-run-1",
            "result": "passed",
            "exit_code": 0,
            "started_at_unix_ms": 1,
            "finished_at_unix_ms": 2,
            "repository_commit": repository_commit,
            "repository_tree": repository_tree,
            "assertions": assertions
        });
        if gate_id == "webui_online_e2e" {
            sign_online_report(&mut report);
        }
        let report_bytes = serde_json::to_vec(&report).expect("encode report");
        fs::write(root.join(&report_path), &report_bytes).expect("write report");
        let artifact = serde_json::json!({
            "node_id": "foundation.root",
            "gate_id": gate_id,
            "command": command,
            "result": "passed",
            "online_run_id": "online-run-1",
            "proof_kind": proof_kind,
            "verifier_report_path": report_path,
            "verifier_report_sha256": format!("{:x}", Sha256::digest(&report_bytes))
        });
        fs::write(
            root.join(&artifact_path),
            serde_json::to_vec(&artifact).expect("encode artifact"),
        )
        .expect("write artifact");
        records.push(serde_json::json!({
            "node_id": "foundation.root",
            "gate_id": gate_id,
            "command": command,
            "result": "passed",
            "online_run_id": "online-run-1",
            "artifact_path": artifact_path
        }));
    }
    (root, serde_json::json!({"evidence": records}), gates)
}

pub(super) fn retain_repository_evidence(
    root: &Path,
    mut node: Value,
    mut gates: BTreeSet<String>,
) -> (Value, BTreeSet<String>) {
    for record in node["evidence"].as_array().expect("evidence") {
        if record["gate_id"] == "openminis_ui_migration_manifest" {
            continue;
        }
        let artifact_path = record["artifact_path"].as_str().expect("artifact path");
        let artifact: Value = serde_json::from_str(
            &fs::read_to_string(root.join(artifact_path)).expect("read artifact"),
        )
        .expect("parse artifact");
        let report_path = artifact["verifier_report_path"]
            .as_str()
            .expect("report path");
        fs::remove_file(root.join(artifact_path)).expect("remove non-repository artifact");
        fs::remove_file(root.join(report_path)).expect("remove non-repository report");
    }
    node["evidence"]
        .as_array_mut()
        .expect("evidence")
        .retain(|record| record["gate_id"] == "openminis_ui_migration_manifest");
    gates.retain(|gate| gate == "openminis_ui_migration_manifest");
    (node, gates)
}

pub(super) fn write_openminis_retirement_fixture(
    name: &str,
    legacy_touched: bool,
) -> (TestRepoRoot, Value, BTreeSet<String>) {
    let root = test_repo_root(&format!("openminis-retirement-{name}"));
    fs::create_dir_all(root.join("scan")).expect("create scan dir");
    fs::create_dir_all(root.join("docs/migrations/openminis-ui/evidence"))
        .expect("create evidence dir");
    fs::create_dir_all(root.join("docs/mainline-calls")).expect("create mainline dir");
    fs::write(
        root.join("scan/current.js"),
        "function currentSymbol() {}\n",
    )
    .expect("write scan source");
    let mainline_path = "docs/mainline-calls/app.webui-smoke.json";
    let mainline = serde_json::json!({
        "feature_id": "app.webui-smoke",
        "mainline_call_doc": mainline_path,
        "legacy_scan_roots": [{
            "node_id": "foundation.root",
            "owner_feature_id": "app.webui-smoke",
            "scan_paths": ["scan"],
            "removed_paths": ["scan/legacy-removed.js"],
            "removed_symbols": ["legacySymbol"],
            "removed_import_tokens": ["legacy/import"],
            "removed_callers": ["legacyCaller"]
        }]
    });
    fs::write(
        root.join(mainline_path),
        serde_json::to_vec(&mainline).expect("encode mainline"),
    )
    .expect("write mainline");
    fs::write(
        root.join("docs/migrations/openminis-ui/ui-tree.manifest.json"),
        serde_json::to_vec(&serde_json::json!({
            "status": "migration_in_progress",
            "nodes": [{
                "node_id": "foundation.root",
                "status": "source_bound",
                "evidence": []
            }]
        }))
        .expect("encode baseline lifecycle manifest"),
    )
    .expect("write baseline lifecycle manifest");
    let (repository_commit, repository_tree) = commit_test_repository_baseline(&root);
    let gate_id = "openminis_ui_legacy_online_no_touch";
    let command = "make verify-webui-online";
    let artifact_path = "docs/migrations/openminis-ui/evidence/no-touch.json";
    let report_path = "docs/migrations/openminis-ui/evidence/no-touch-report.json";
    let mut report = serde_json::json!({
        "schema_version": "freehand.verifier-report.v1",
        "verifier_id": "freehand.webui_online.legacy_no_touch",
        "node_id": "foundation.root",
        "migration_unit_id": "ui_migration.foundation.root",
        "command": command,
        "online_run_id": "online-retirement-1",
        "result": "passed",
        "exit_code": 0,
        "started_at_unix_ms": 1,
        "finished_at_unix_ms": 2,
        "repository_commit": repository_commit,
        "repository_tree": repository_tree,
        "assertions": {
            "daemon_hosted": true,
            "legacy_not_loaded": true,
            "owner_truth_verified": true
        }
    });
    sign_online_report(&mut report);
    let report_bytes = serde_json::to_vec(&report).expect("encode report");
    fs::write(root.join(report_path), &report_bytes).expect("write report");
    let artifact = serde_json::json!({
        "node_id": "foundation.root",
        "gate_id": gate_id,
        "command": command,
        "result": "passed",
        "online_run_id": "online-retirement-1",
        "legacy_touched": legacy_touched,
        "proof_kind": "legacy_online_no_touch",
        "verifier_report_path": report_path,
        "verifier_report_sha256": format!("{:x}", Sha256::digest(&report_bytes))
    });
    fs::write(
        root.join(artifact_path),
        serde_json::to_vec(&artifact).expect("encode artifact"),
    )
    .expect("write artifact");
    let node = serde_json::json!({
        "owner_feature_id": "app.webui-smoke",
        "mainline_call_docs": [mainline_path],
        "target_paths": ["scan/current.js"],
        "evidence": [{
            "node_id": "foundation.root",
            "gate_id": gate_id,
            "command": command,
            "result": "passed",
            "online_run_id": "online-retirement-1",
            "artifact_path": artifact_path
        }],
        "legacy_retirement": {
            "required": true,
            "scan_paths": ["scan"],
            "removed_paths": ["scan/legacy-removed.js"],
            "removed_symbols": ["legacySymbol"],
            "removed_import_tokens": ["legacy/import"],
            "removed_callers": ["legacyCaller"],
            "online_no_touch_gate_id": gate_id
        }
    });
    let gates = [gate_id.to_owned()].into_iter().collect::<BTreeSet<_>>();
    (root, node, gates)
}

fn sign_online_report(report: &mut Value) {
    report["provenance_key_id"] = Value::String("freehand.openminis-online.v1".to_owned());
    let payload =
        super::super::evidence::provenance::canonical_report_payload(report).expect("payload");
    let signature = SigningKey::from_bytes(&[7_u8; 32]).sign(&payload);
    report["provenance_signature"] = Value::String(hex::encode(signature.to_bytes()));
}
