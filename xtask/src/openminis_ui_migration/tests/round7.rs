use super::super::*;
use super::common::*;

mod round7_declarations;
use sha2::{Digest, Sha256};

#[test]
fn openminis_ui_migration_manifest_accepts_matching_structured_evidence() {
    let (root, node, gates) = write_openminis_evidence_fixture("matching");
    let (node, gates) = retain_repository_evidence(&root, node, gates);
    verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect("matching structured evidence should pass");
}

#[test]
fn openminis_ui_migration_manifest_accepts_committed_evidence_over_attested_source() {
    let (root, node, gates) = write_openminis_evidence_fixture("committed-evidence");
    let (node, gates) = retain_repository_evidence(&root, node, gates);
    run_test_git(&root, &["add", "docs/migrations/openminis-ui/evidence"]);
    run_test_git(&root, &["commit", "-m", "evidence"]);
    verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect("committed artifact/report changes over the attested source must pass");
}

#[test]
fn openminis_ui_migration_declarations_reject_partial_syntax_trees() {
    let source = "function forgedByPartialParse() {}\nconst broken = ;\n";
    let err = declared_symbols(Path::new("malformed.js"), source)
        .expect_err("error-bearing syntax trees must not provide declaration truth");
    assert!(err.contains("syntax-error tree"), "{err}");
}

#[test]
fn openminis_ui_migration_target_symbol_requires_one_declaration() {
    let (root, node) =
        write_openminis_binding_fixture("duplicate-declaration", "exact", "bound", "bound");
    fs::write(
        root.join("src/target.js"),
        "function exactSymbol() {}\nfunction exactSymbol() {}\n",
    )
    .expect("write two declarations in one target file");

    let err = verify_openminis_ui_target_bindings(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        "ui_projection.render",
    )
    .expect_err("duplicate target declarations must fail");
    assert!(err.contains("exactly one declaration"), "{err}");
    assert!(err.contains("found 2"), "{err}");
}

#[test]
fn openminis_ui_migration_target_binding_ignores_non_source_assets() {
    let (root, mut node) =
        write_openminis_binding_fixture("mixed-android-assets", "exact", "bound", "bound");
    let bridge = root.join("src/android");
    fs::create_dir_all(&bridge).expect("create Android source tree");
    fs::write(bridge.join("Bridge.kt"), "class ExactBridge\n").expect("write Kotlin source");
    fs::write(bridge.join("launcher.png"), [0xff, 0x00, 0x89, 0x50])
        .expect("write binary launcher asset");
    fs::write(bridge.join("layout.xml"), "<layout />\n").expect("write XML asset");
    node["target_paths"] = serde_json::json!(["src/android"]);
    node["target_symbols"] = serde_json::json!(["ExactBridge"]);
    let mainline = serde_json::json!({
        "call_table": [{
            "symbol_path": "ExactBridge",
            "file_path": "src/android/Bridge.kt",
            "resource_operation": "ui_projection.render",
            "binding_status": "bound"
        }]
    });
    fs::write(
        root.join("docs/mainline-calls/test.json"),
        serde_json::to_vec(&mainline).expect("encode mainline"),
    )
    .expect("write Android mainline");

    verify_openminis_ui_target_bindings(
        &root,
        "platform.android_bridge",
        node.as_object().expect("node"),
        "ui_projection.render",
    )
    .expect("source declarations must be resolved without parsing Android assets");
}

#[test]
fn openminis_ui_migration_pinned_symbol_requires_one_declaration() {
    let root = test_repo_root("duplicate-pinned-declaration");
    let repository = root.join("external/OpenMinis");
    fs::create_dir_all(repository.join("src/ios/Views")).expect("create source repo");
    run_test_git(&repository, &["init"]);
    run_test_git(
        &repository,
        &["config", "user.email", "xtask@example.invalid"],
    );
    run_test_git(&repository, &["config", "user.name", "xtask"]);
    fs::write(
        repository.join("src/ios/Views/Duplicates.swift"),
        "struct OuterA { struct DuplicateView {} }\nstruct OuterB { struct DuplicateView {} }\n",
    )
    .expect("write two declarations in one pinned file");
    run_test_git(&repository, &["add", "."]);
    run_test_git(&repository, &["commit", "-m", "pinned"]);
    let pinned = git_stdout(&repository, &["rev-parse", "HEAD"]).expect("pinned commit");
    fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
    fs::write(
        root.join(".github/workflows/ci.yml"),
        format!(
            "jobs:\n  gate:\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          repository: OpenMinis/OpenMinis\n          ref: {pinned}\n          path: external/OpenMinis\n      - uses: swift-actions/setup-swift@v2\n      - run: make ci\n"
        ),
    )
    .expect("write ci");
    let source_repository = serde_json::json!({
        "path_hint": "external/OpenMinis",
        "commit": pinned
    });
    let nodes = serde_json::json!([{
        "node_id": "foundation.root",
        "source_paths": ["src/ios/Views"],
        "source_symbols": ["DuplicateView"]
    }]);

    let err = verify_openminis_ui_pinned_source(
        &root,
        source_repository.as_object().expect("source repository"),
        nodes.as_array().expect("nodes"),
    )
    .expect_err("duplicate pinned declarations must fail");
    assert!(err.contains("exactly one declaration"), "{err}");
    assert!(err.contains("found 2"), "{err}");
}

#[test]
fn openminis_ui_migration_maps_equal_touched_features() {
    let root = write_map_fixture();
    let node = serde_json::json!({
        "function_map_docs": ["docs/function-maps/owner.feature.md"],
        "mainline_call_docs": ["docs/mainline-calls/owner.feature.json"],
        "test_design_docs": ["docs/testing/owner.feature.md"]
    });
    let touched_only = BTreeSet::from(["owner.feature".to_owned(), "touched.only".to_owned()]);
    let err = verify_openminis_ui_map_documents(
        &root,
        "foundation.root",
        "owner.feature",
        &touched_only,
        node.as_object().expect("node"),
    )
    .expect_err("touched-only feature must fail");
    assert!(err.contains("must equal touched_feature_ids"), "{err}");

    let map_only = BTreeSet::from(["owner.feature".to_owned()]);
    let mut node = node;
    for field in [
        "function_map_docs",
        "mainline_call_docs",
        "test_design_docs",
    ] {
        node[field]
            .as_array_mut()
            .expect("map paths")
            .push(Value::String(
                match field {
                    "function_map_docs" => "docs/function-maps/map.only.md",
                    "mainline_call_docs" => "docs/mainline-calls/map.only.json",
                    _ => "docs/testing/map.only.md",
                }
                .to_owned(),
            ));
    }
    let err = verify_openminis_ui_map_documents(
        &root,
        "foundation.root",
        "owner.feature",
        &map_only,
        node.as_object().expect("node"),
    )
    .expect_err("map-only feature must fail");
    assert!(err.contains("must equal touched_feature_ids"), "{err}");
}

#[test]
fn openminis_ui_migration_rejects_verifier_report_truth_drift() {
    for (name, field, value, expected) in [
        (
            "verifier-id",
            "verifier_id",
            Value::String("forged.verifier".to_owned()),
            "field `verifier_id` drift",
        ),
        (
            "command",
            "command",
            Value::String("true".to_owned()),
            "field `command` drift",
        ),
        (
            "run-id",
            "online_run_id",
            Value::String("forged-run".to_owned()),
            "field `online_run_id` drift",
        ),
        (
            "node-id",
            "node_id",
            Value::String("home.dashboard".to_owned()),
            "field `node_id` drift",
        ),
        (
            "migration-unit-id",
            "migration_unit_id",
            Value::String("ui_migration.home.dashboard".to_owned()),
            "field `migration_unit_id` drift",
        ),
        (
            "exit-code",
            "exit_code",
            Value::Number(1.into()),
            "not a successful process result",
        ),
    ] {
        let (root, node, gates) = write_openminis_evidence_fixture(name);
        rewrite_first_verifier_report(&root, &node, |report| report[field] = value);
        let err = verify_openminis_ui_evidence(
            &root,
            "foundation.root",
            node.as_object().expect("node"),
            &gates,
        )
        .expect_err("verifier report truth drift must fail");
        assert!(err.contains(expected), "{name}: {err}");
    }

    let (root, node, gates) = write_openminis_evidence_fixture("digest-drift");
    let artifact_path = root.join(first_artifact_path(&node));
    let artifact: Value =
        serde_json::from_str(&fs::read_to_string(&artifact_path).expect("read artifact"))
            .expect("parse artifact");
    let report_path = artifact["verifier_report_path"]
        .as_str()
        .expect("report path");
    fs::write(root.join(report_path), "{}").expect("drift report without digest update");
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("verifier report digest drift must fail");
    assert!(err.contains("verifier report digest drift"), "{err}");

    let (root, node, gates) = write_openminis_evidence_fixture("assertion-false");
    rewrite_first_verifier_report(&root, &node, |report| {
        let assertion = report["assertions"]
            .as_object_mut()
            .expect("assertions")
            .keys()
            .next()
            .expect("assertion")
            .to_owned();
        report["assertions"][&assertion] = Value::Bool(false);
    });
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("false raw report assertion must fail");
    assert!(err.contains("lacks passed assertion"), "{err}");
}

#[test]
fn openminis_ui_migration_repository_evidence_uses_non_recursive_node_verifier() {
    let (root, node, gates) = write_openminis_evidence_fixture("node-verifier-command");
    let (node, gates) = retain_repository_evidence(&root, node, gates);
    verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect("node verifier evidence should pass");
    for record in node["evidence"].as_array().expect("evidence") {
        if matches!(
            record["gate_id"].as_str(),
            Some("openminis_ui_migration_manifest" | "cargo_run_xtask_gates_check")
        ) {
            assert_eq!(
                record["command"],
                "cargo run -p xtask -- openminis-ui verify-node foundation.root"
            );
            assert_ne!(record["command"], "cargo run -p xtask -- gates check");
        }
    }
}

#[test]
fn openminis_ui_migration_rejects_stale_revision_or_dirty_source_evidence() {
    for (name, field, expected) in [
        (
            "stale-commit",
            "repository_commit",
            "repository_commit `0000000000000000000000000000000000000000` must resolve to a commit object",
        ),
        (
            "symbolic-commit",
            "repository_commit",
            "repository_commit must be a full 40-character git SHA",
        ),
        ("stale-tree", "repository_tree", "repository tree drift"),
    ] {
        let (root, node, gates) = write_openminis_evidence_fixture(name);
        let (node, gates) = retain_repository_evidence(&root, node, gates);
        rewrite_first_verifier_report(&root, &node, |report| {
            report[field] = Value::String(
                if name == "symbolic-commit" {
                    "HEAD"
                } else {
                    "0000000000000000000000000000000000000000"
                }
                .to_owned(),
            );
        });
        let err = verify_openminis_ui_evidence(
            &root,
            "foundation.root",
            node.as_object().expect("node"),
            &gates,
        )
        .expect_err("stale source revision must fail");
        assert!(err.contains(expected), "{err}");
    }

    let (root, node, gates) = write_openminis_evidence_fixture("dirty-source");
    let (node, gates) = retain_repository_evidence(&root, node, gates);
    fs::create_dir_all(root.join("src")).expect("create source dir");
    fs::write(root.join("src/changed.rs"), "fn changed() {}\n").expect("write dirty source");
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("dirty non-evidence source must fail");
    assert!(err.contains("cannot attest dirty repository path"), "{err}");
    assert!(err.contains("src/changed.rs"), "{err}");

    let (root, node, gates) = write_openminis_evidence_fixture("committed-dirty-source");
    let (node, gates) = retain_repository_evidence(&root, node, gates);
    fs::create_dir_all(root.join("src")).expect("create source dir");
    fs::write(root.join("src/changed.rs"), "fn changed() {}\n").expect("write dirty source");
    run_test_git(&root, &["add", "src/changed.rs"]);
    run_test_git(&root, &["commit", "-m", "source drift"]);
    let err = verify_openminis_ui_evidence(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("committed source after the attested revision must fail");
    assert!(err.contains("cannot attest dirty repository path"), "{err}");
    assert!(err.contains("src/changed.rs"), "{err}");
}

#[test]
fn openminis_ui_migration_phase_closes_only_after_retirement() {
    let online = vec![serde_json::json!({"status": "online_verified"})];
    verify_openminis_ui_manifest_phase("migration_in_progress", &online)
        .expect("all-online migration remains in progress");
    verify_openminis_ui_manifest_phase("migration_complete", &online)
        .expect_err("all-online migration is not retired");

    let retired = vec![serde_json::json!({"status": "legacy_retired"})];
    verify_openminis_ui_manifest_phase("migration_complete", &retired)
        .expect("all-retired migration is complete");
}

fn write_map_fixture() -> TestRepoRoot {
    let root = test_repo_root("map-touched-equality");
    for directory in ["docs/function-maps", "docs/mainline-calls", "docs/testing"] {
        fs::create_dir_all(root.join(directory)).expect("create map directory");
    }
    for feature in ["owner.feature", "map.only"] {
        fs::write(
            root.join(format!("docs/function-maps/{feature}.md")),
            format!("# Function Map: `{feature}`\n\n- feature_id: `{feature}`\n"),
        )
        .expect("write function map");
        fs::write(
            root.join(format!("docs/testing/{feature}.md")),
            format!("# Test Design: `{feature}`\n\n- feature_id: `{feature}`\n"),
        )
        .expect("write test design");
        let path = format!("docs/mainline-calls/{feature}.json");
        fs::write(
            root.join(&path),
            serde_json::to_vec(&serde_json::json!({
                "feature_id": feature,
                "mainline_call_doc": path,
                "function_map_doc": format!("docs/function-maps/{feature}.md"),
                "test_design_doc": format!("docs/testing/{feature}.md")
            }))
            .expect("encode mainline"),
        )
        .expect("write mainline");
    }
    root
}

fn first_artifact_path(node: &Value) -> PathBuf {
    PathBuf::from(
        node["evidence"][0]["artifact_path"]
            .as_str()
            .expect("artifact path"),
    )
}

fn rewrite_first_verifier_report(root: &Path, node: &Value, mutate: impl FnOnce(&mut Value)) {
    let artifact_path = root.join(first_artifact_path(node));
    let mut artifact: Value =
        serde_json::from_str(&fs::read_to_string(&artifact_path).expect("read artifact"))
            .expect("parse artifact");
    let report_path = artifact["verifier_report_path"]
        .as_str()
        .expect("report path");
    let mut report: Value =
        serde_json::from_str(&fs::read_to_string(root.join(report_path)).expect("read report"))
            .expect("parse report");
    mutate(&mut report);
    let bytes = serde_json::to_vec(&report).expect("encode report");
    fs::write(root.join(report_path), &bytes).expect("write report");
    artifact["verifier_report_sha256"] = Value::String(format!("{:x}", Sha256::digest(&bytes)));
    fs::write(
        artifact_path,
        serde_json::to_vec(&artifact).expect("encode artifact"),
    )
    .expect("write artifact");
}
