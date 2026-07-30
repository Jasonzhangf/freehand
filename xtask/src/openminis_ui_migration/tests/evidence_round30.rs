use super::super::*;
use super::common::*;
use sha2::{Digest, Sha256};

#[test]
fn openminis_ui_evidence_allows_only_canonical_manifest_transition_metadata() {
    let (root, node, gates) = write_openminis_evidence_fixture("manifest-transition");
    let (node, gates) = retain_repository_evidence(&root, node, gates);
    let manifest_path = root.join("docs/migrations/openminis-ui/ui-tree.manifest.json");
    let mut manifest: Value =
        serde_json::from_slice(&fs::read(&manifest_path).expect("read lifecycle manifest"))
            .expect("parse lifecycle manifest");
    manifest["nodes"][0]["status"] = Value::String("online_verified".to_owned());
    manifest["nodes"][0]["evidence"] = serde_json::json!([{"gate_id": "webui_online_e2e"}]);
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest).expect("encode lifecycle transition"),
    )
    .expect("write lifecycle transition");

    verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect("canonical lifecycle manifest metadata may follow the attested source revision");

    manifest["nodes"][0]["operation_id"] =
        Value::String("ui_projection.forged_operation".to_owned());
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest).expect("encode contract drift"),
    )
    .expect("write contract drift");
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("manifest operation contract drift must not reuse prior evidence");
    assert!(err.contains("non-lifecycle contract drift"), "{err}");

    manifest["nodes"][0]["operation_id"] =
        Value::String("ui_projection.render_foundation".to_owned());
    fs::write(
        &manifest_path,
        serde_json::to_vec(&manifest).expect("restore lifecycle transition"),
    )
    .expect("restore lifecycle transition");
    fs::create_dir_all(root.join("src")).expect("create source directory");
    fs::write(root.join("src/runtime.rs"), "fn changed() {}\n").expect("write source drift");
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("runtime source drift must remain outside lifecycle metadata admission");
    assert!(err.contains("src/runtime.rs"), "{err}");
}

#[test]
fn openminis_ui_evidence_rejects_tree_object_as_repository_commit() {
    let (root, node, gates) = write_openminis_evidence_fixture("tree-object-revision");
    let (node, gates) = retain_repository_evidence(&root, node, gates);
    let artifact_path = node["evidence"][0]["artifact_path"]
        .as_str()
        .expect("artifact path");
    let mut artifact: Value =
        serde_json::from_slice(&fs::read(root.join(artifact_path)).expect("read artifact"))
            .expect("parse artifact");
    let report_path = artifact["verifier_report_path"]
        .as_str()
        .expect("report path");
    let mut report: Value =
        serde_json::from_slice(&fs::read(root.join(report_path)).expect("read report"))
            .expect("parse report");
    report["repository_commit"] = report["repository_tree"].clone();
    let report_bytes = serde_json::to_vec(&report).expect("encode forged tree report");
    fs::write(root.join(report_path), &report_bytes).expect("write forged tree report");
    artifact["verifier_report_sha256"] =
        Value::String(format!("{:x}", Sha256::digest(&report_bytes)));
    fs::write(
        root.join(artifact_path),
        serde_json::to_vec(&artifact).expect("encode artifact"),
    )
    .expect("write artifact");

    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("tree object must not attest a committed verifier revision");
    assert!(err.contains("must resolve to a commit object"), "{err}");
}
