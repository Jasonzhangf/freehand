use super::super::*;
use super::common::*;

#[test]
fn openminis_ui_migration_manifest_rejects_self_reported_noncanonical_proof() {
    let (root, mut node, gates) = write_openminis_evidence_fixture("forged-command");
    node["evidence"][0]["command"] = Value::String("true".to_owned());
    let artifact_path = node["evidence"][0]["artifact_path"]
        .as_str()
        .expect("artifact");
    let mut artifact: Value =
        serde_json::from_str(&fs::read_to_string(root.join(artifact_path)).expect("read artifact"))
            .expect("parse artifact");
    artifact["command"] = Value::String("true".to_owned());
    fs::write(
        root.join(artifact_path),
        serde_json::to_vec(&artifact).expect("artifact bytes"),
    )
    .expect("write artifact");
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("self-reported true command must fail");
    assert!(
        err.contains("not a passed executable online proof"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_manifest_rejects_self_reported_online_success() {
    let (root, node, _) = write_openminis_evidence_fixture("self-reported-online-success");
    let webui_record = node["evidence"]
        .as_array()
        .expect("evidence")
        .iter()
        .find(|record| record["gate_id"] == "webui_online_e2e")
        .expect("WebUI record")
        .clone();
    let node = serde_json::json!({"evidence": [webui_record]});
    let gates = BTreeSet::from(["webui_online_e2e".to_owned()]);

    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("repository-authored online success must not promote lifecycle truth");
    assert!(
        err.contains("no source-bound external provenance verifier"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_manifest_rejects_evidence_outside_dedicated_root() {
    let (root, mut node, gates) = write_openminis_evidence_fixture("artifact-root");
    fs::create_dir_all(root.join("src")).expect("create src");
    let original = node["evidence"][0]["artifact_path"]
        .as_str()
        .expect("artifact path");
    fs::copy(root.join(original), root.join("src/forged-artifact.json")).expect("copy artifact");
    node["evidence"][0]["artifact_path"] = Value::String("src/forged-artifact.json".to_owned());
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("artifact outside dedicated evidence root must fail");
    assert!(err.contains("artifact_path must be under"), "{err}");

    let (root, node, gates) = write_openminis_evidence_fixture("report-root");
    fs::create_dir_all(root.join("src")).expect("create src");
    let artifact_path = root.join(node["evidence"][0]["artifact_path"].as_str().expect("path"));
    let mut artifact: Value =
        serde_json::from_str(&fs::read_to_string(&artifact_path).expect("read artifact"))
            .expect("parse artifact");
    let report_path = artifact["verifier_report_path"]
        .as_str()
        .expect("report path");
    fs::copy(root.join(report_path), root.join("src/forged-report.json")).expect("copy report");
    artifact["verifier_report_path"] = Value::String("src/forged-report.json".to_owned());
    fs::write(
        artifact_path,
        serde_json::to_vec(&artifact).expect("encode artifact"),
    )
    .expect("write artifact");
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("report outside dedicated evidence root must fail");
    assert!(err.contains("verifier_report_path must be under"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_evidence_identity_and_json_drift() {
    let (root, mut node, gates) = write_openminis_evidence_fixture("identity-drift");
    node["evidence"][0]
        .as_object_mut()
        .expect("evidence")
        .remove("node_id");
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("missing evidence node id must fail");
    assert!(err.contains("missing non-empty string `node_id`"), "{err}");

    let (root, mut node, gates) = write_openminis_evidence_fixture("missing-run-id");
    node["evidence"][0]
        .as_object_mut()
        .expect("evidence")
        .remove("online_run_id");
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("missing online run id must fail");
    assert!(
        err.contains("missing non-empty string `online_run_id`"),
        "{err}"
    );

    let (root, node, gates) = write_openminis_evidence_fixture("run-id-drift");
    let path = root.join(node["evidence"][0]["artifact_path"].as_str().expect("path"));
    let mut artifact: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read artifact"))
            .expect("parse artifact");
    artifact["online_run_id"] = Value::String("wrong-run".to_owned());
    fs::write(
        &path,
        serde_json::to_vec(&artifact).expect("encode artifact"),
    )
    .expect("write artifact");
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("artifact online run drift must fail");
    assert!(err.contains("field `online_run_id` drift"), "{err}");

    let (root, node, gates) = write_openminis_evidence_fixture("invalid-json");
    let path = root.join(node["evidence"][0]["artifact_path"].as_str().expect("path"));
    fs::write(path, "not-json").expect("corrupt artifact");
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("non-json evidence artifact must fail");
    assert!(err.contains("is not valid JSON"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_evidence_artifact_or_coverage_drift() {
    let (root, node, gates) = write_openminis_evidence_fixture("artifact-drift");
    let path = root.join(node["evidence"][0]["artifact_path"].as_str().expect("path"));
    let mut artifact: Value =
        serde_json::from_str(&fs::read_to_string(&path).expect("read artifact"))
            .expect("parse artifact");
    artifact["command"] = Value::String("wrong command".to_owned());
    fs::write(
        &path,
        serde_json::to_vec(&artifact).expect("encode artifact"),
    )
    .expect("write artifact");
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("artifact command drift must fail");
    assert!(err.contains("field `command` drift"), "{err}");

    let (root, mut node, gates) = write_openminis_evidence_fixture("coverage-drift");
    node["evidence"].as_array_mut().expect("evidence").pop();
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("incomplete gate coverage must fail");
    assert!(err.contains("evidence gate coverage drift"), "{err}");
}

#[test]
fn openminis_ui_migration_legacy_retirement_accepts_complete_structural_proof() {
    let (root, node, gates) = write_openminis_retirement_fixture("complete", false);
    verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect("complete retirement should pass");
}

#[test]
fn openminis_ui_migration_legacy_retirement_rejects_fabricated_removed_identity() {
    let (root, mut node, gates) = write_openminis_retirement_fixture("fabricated-identity", false);
    node["legacy_retirement"]["removed_symbols"] = serde_json::json!(["neverExisted"]);
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("manifest-selected removed identity must fail");
    assert!(
        err.contains("removed_symbols drift from owner registry"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_manifest_rejects_unowned_or_uncovered_legacy_scan_roots() {
    let (root, mut node, gates) = write_openminis_retirement_fixture("empty-scan-root", false);
    fs::create_dir_all(root.join("empty")).expect("create empty scan dir");
    node["legacy_retirement"]["scan_paths"] = serde_json::json!(["empty"]);
    let mainline_path = root.join("docs/mainline-calls/app.webui-smoke.json");
    let mut mainline: Value =
        serde_json::from_str(&fs::read_to_string(&mainline_path).expect("read mainline"))
            .expect("parse mainline");
    mainline["legacy_scan_roots"][0]["scan_paths"] = serde_json::json!(["empty"]);
    fs::write(
        &mainline_path,
        serde_json::to_vec(&mainline).expect("encode mainline"),
    )
    .expect("write mainline");
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("arbitrary in-repository empty scan root must fail");
    assert!(err.contains("do not cover bound target_path"), "{err}");

    let (root, node, gates) = write_openminis_retirement_fixture("missing-registry", false);
    let mainline_path = root.join("docs/mainline-calls/app.webui-smoke.json");
    let mut mainline: Value =
        serde_json::from_str(&fs::read_to_string(&mainline_path).expect("read mainline"))
            .expect("parse mainline");
    mainline["legacy_scan_roots"] = serde_json::json!([]);
    fs::write(
        &mainline_path,
        serde_json::to_vec(&mainline).expect("encode mainline"),
    )
    .expect("write mainline");
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("missing owner registry row must fail");
    assert!(err.contains("exactly one owner-bound"), "{err}");

    let (root, node, gates) = write_openminis_retirement_fixture("duplicate-registry", false);
    let mainline_path = root.join("docs/mainline-calls/app.webui-smoke.json");
    let mut mainline: Value =
        serde_json::from_str(&fs::read_to_string(&mainline_path).expect("read mainline"))
            .expect("parse mainline");
    let duplicate = mainline["legacy_scan_roots"][0].clone();
    mainline["legacy_scan_roots"]
        .as_array_mut()
        .expect("scan roots")
        .push(duplicate);
    fs::write(
        &mainline_path,
        serde_json::to_vec(&mainline).expect("encode mainline"),
    )
    .expect("write mainline");
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("duplicate owner registry rows must fail");
    assert!(err.contains("exactly one owner-bound"), "{err}");

    let (root, node, gates) = write_openminis_retirement_fixture("wrong-mainline-id", false);
    let mainline_path = root.join("docs/mainline-calls/app.webui-smoke.json");
    let mut mainline: Value =
        serde_json::from_str(&fs::read_to_string(&mainline_path).expect("read mainline"))
            .expect("parse mainline");
    mainline["mainline_call_doc"] = Value::String("docs/mainline-calls/other.json".to_owned());
    fs::write(
        &mainline_path,
        serde_json::to_vec(&mainline).expect("encode mainline"),
    )
    .expect("write mainline");
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("noncanonical owner mainline identity must fail");
    assert!(err.contains("canonical machine mainline"), "{err}");

    let (root, node, gates) = write_openminis_retirement_fixture("wrong-owner", false);
    let mainline_path = root.join("docs/mainline-calls/app.webui-smoke.json");
    let mut mainline: Value =
        serde_json::from_str(&fs::read_to_string(&mainline_path).expect("read mainline"))
            .expect("parse mainline");
    mainline["legacy_scan_roots"][0]["owner_feature_id"] = Value::String("ui.protocol".to_owned());
    fs::write(
        &mainline_path,
        serde_json::to_vec(&mainline).expect("encode mainline"),
    )
    .expect("write mainline");
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("wrong owner registry row must fail");
    assert!(err.contains("legacy_scan_roots owner drift"), "{err}");

    let (root, mut node, gates) = write_openminis_retirement_fixture("non-directory", false);
    node["legacy_retirement"]["scan_paths"] = serde_json::json!(["scan/current.js"]);
    let mainline_path = root.join("docs/mainline-calls/app.webui-smoke.json");
    let mut mainline: Value =
        serde_json::from_str(&fs::read_to_string(&mainline_path).expect("read mainline"))
            .expect("parse mainline");
    mainline["legacy_scan_roots"][0]["scan_paths"] = serde_json::json!(["scan/current.js"]);
    fs::write(
        &mainline_path,
        serde_json::to_vec(&mainline).expect("encode mainline"),
    )
    .expect("write mainline");
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("file scan root must fail");
    assert!(err.contains("must be a directory"), "{err}");

    let (root, mut node, gates) = write_openminis_retirement_fixture("registry-drift", false);
    fs::create_dir_all(root.join("other")).expect("create other dir");
    node["legacy_retirement"]["scan_paths"] = serde_json::json!(["other"]);
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("manifest-selected scan root must fail owner registry match");
    assert!(
        err.contains("scan_paths drift from owner registry"),
        "{err}"
    );

    let (root, mut node, gates) = write_openminis_retirement_fixture("removed-uncovered", false);
    node["legacy_retirement"]["removed_paths"] = serde_json::json!(["elsewhere/legacy.js"]);
    let mainline_path = root.join("docs/mainline-calls/app.webui-smoke.json");
    let mut mainline: Value =
        serde_json::from_str(&fs::read_to_string(&mainline_path).expect("read mainline"))
            .expect("parse mainline");
    mainline["legacy_scan_roots"][0]["removed_paths"] = serde_json::json!(["elsewhere/legacy.js"]);
    fs::write(
        &mainline_path,
        serde_json::to_vec(&mainline).expect("encode mainline"),
    )
    .expect("write mainline");
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("removed path outside owner scan roots must fail");
    assert!(err.contains("do not cover removed_path"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_remaining_legacy_or_touch() {
    let (root, node, gates) = write_openminis_retirement_fixture("remaining-path", false);
    fs::write(root.join("scan/legacy-removed.js"), "old").expect("write remaining path");
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("remaining legacy path must fail");
    assert!(err.contains("legacy path still exists"), "{err}");

    let (root, node, gates) = write_openminis_retirement_fixture("remaining-symbol", false);
    fs::write(root.join("scan/current.js"), "legacySymbol()").expect("write symbol");
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("remaining legacy symbol must fail");
    assert!(err.contains("legacy symbol still resolves"), "{err}");

    for (name, content, expected) in [
        (
            "remaining-import",
            "import 'legacy/import';",
            "legacy import token still resolves",
        ),
        (
            "remaining-caller",
            "legacyCaller();",
            "legacy caller still resolves",
        ),
    ] {
        let (root, node, gates) = write_openminis_retirement_fixture(name, false);
        fs::write(root.join("scan/current.js"), content).expect("write remaining token");
        let err = verify_openminis_ui_legacy_retirement(
            &root,
            "foundation.root",
            node.as_object().expect("node"),
            &gates,
        )
        .expect_err("remaining legacy token must fail");
        assert!(err.contains(expected), "{err}");
    }

    let (root, mut node, gates) = write_openminis_retirement_fixture("absolute-scan-path", false);
    node["legacy_retirement"]["scan_paths"] = serde_json::json!(["/tmp"]);
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("absolute legacy scan path must fail");
    assert!(err.contains("repository-relative path"), "{err}");

    let (root, node, gates) = write_openminis_retirement_fixture("legacy-touch", true);
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("legacy_touched=true must fail");
    assert!(err.contains("legacy_touched=false"), "{err}");
}
