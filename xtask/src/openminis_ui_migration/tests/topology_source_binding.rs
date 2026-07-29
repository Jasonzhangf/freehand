use super::super::*;
use super::common::*;

#[test]
fn openminis_ui_migration_manifest_rejects_human_entrypoint_drift() {
    let (root, manifest) = openminis_ui_migration_test_manifest();
    let temp = test_repo_root("openminis-entrypoint-drift");
    fs::create_dir_all(temp.join("docs/migrations/openminis-ui")).expect("create migration docs");
    let human = fs::read_to_string(root.join("docs/migrations/openminis-ui/ui-tree.md"))
        .expect("read human tree")
        .replace(
            "- entrypoint_node_id: `foundation.root`",
            "- entrypoint_node_id: `home.dashboard`",
        );
    fs::write(temp.join("docs/migrations/openminis-ui/ui-tree.md"), human)
        .expect("write human tree");
    let object = manifest.as_object().expect("manifest object");
    let node_ids = manifest_node_ids(&manifest);

    let err = verify_openminis_ui_tree_topology(&temp, object, &node_ids)
        .expect_err("human entrypoint drift must fail");
    assert!(err.contains("human entrypoint drift"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_unreachable_required_node() {
    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    manifest["edges"]
        .as_array_mut()
        .expect("edges")
        .retain(|edge| edge["to_node_id"] != "home.dashboard");
    let object = manifest.as_object().expect("manifest object");
    let node_ids = manifest_node_ids(&manifest);

    let err = verify_openminis_ui_tree_topology(&root, object, &node_ids)
        .expect_err("required node without an entrypoint path must fail");
    assert!(err.contains("unreachable from `foundation.root`"), "{err}");
    assert!(err.contains("home.dashboard"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_human_return_semantic_drift() {
    let (root, manifest) = openminis_ui_migration_test_manifest();
    let temp = test_repo_root("openminis-return-drift");
    fs::create_dir_all(temp.join("docs/migrations/openminis-ui")).expect("create migration docs");
    let human = fs::read_to_string(root.join("docs/migrations/openminis-ui/ui-tree.md"))
        .expect("read human tree")
        .replacen("`route_back_to_app_shell`", "`wrong_return_semantic`", 1);
    fs::write(temp.join("docs/migrations/openminis-ui/ui-tree.md"), human)
        .expect("write human tree");
    let object = manifest.as_object().expect("manifest object");
    let node_ids = manifest_node_ids(&manifest);

    let err = verify_openminis_ui_tree_topology(&temp, object, &node_ids)
        .expect_err("human return semantic drift must fail");
    assert!(err.contains("human return-path registry drift"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_human_forward_semantic_drift() {
    let (root, manifest) = openminis_ui_migration_test_manifest();
    let temp = test_repo_root("openminis-forward-semantic-drift");
    fs::create_dir_all(temp.join("docs/migrations/openminis-ui")).expect("create migration docs");
    let human = fs::read_to_string(root.join("docs/migrations/openminis-ui/ui-tree.md"))
        .expect("read human tree")
        .replacen("`contains_or_navigates_to`", "`wrong_forward_semantic`", 1);
    fs::write(temp.join("docs/migrations/openminis-ui/ui-tree.md"), human)
        .expect("write human tree");
    let object = manifest.as_object().expect("manifest object");
    let node_ids = manifest_node_ids(&manifest);

    let err = verify_openminis_ui_tree_topology(&temp, object, &node_ids)
        .expect_err("human forward semantic drift must fail");
    assert!(err.contains("human forward-edge registry drift"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_excluded_source_path() {
    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    manifest["nodes"][0]["source_paths"] =
        serde_json::json!(["src/ios/Views/BrowserUse/BrowserSheetView.swift"]);

    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("BrowserUse source path must fail");
    assert!(err.contains("excluded source path"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_recursively_resolved_excluded_source_path() {
    let root = test_repo_root("openminis-recursive-excluded-source");
    let repository = root.join("external/OpenMinis");
    fs::create_dir_all(repository.join("src/ios/Agent/BrowserUse")).expect("create source repo");
    fs::create_dir_all(repository.join("src/ios/Views")).expect("create safe source");
    run_test_git(&repository, &["init"]);
    run_test_git(
        &repository,
        &["config", "user.email", "xtask@example.invalid"],
    );
    run_test_git(&repository, &["config", "user.name", "xtask"]);
    fs::write(
        repository.join("src/ios/Views/SafeView.swift"),
        "struct SafeView {}\n",
    )
    .expect("write safe source");
    fs::write(
        repository.join("src/ios/Agent/BrowserUse/BrowserSheetView.swift"),
        "struct BrowserSheetView {}\n",
    )
    .expect("write excluded descendant");
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
        "source_paths": ["src/ios"],
        "source_symbols": ["SafeView"]
    }]);

    let err = verify_openminis_ui_pinned_source(
        &root,
        source_repository.as_object().expect("source repository"),
        nodes.as_array().expect("nodes"),
    )
    .expect_err("excluded descendant under an allowed ancestor must fail");
    assert!(
        err.contains("recursively resolves excluded source path"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_manifest_rejects_missing_pinned_source_symbol() {
    let (root, mut manifest) = openminis_ui_migration_test_manifest();
    manifest["nodes"][0]["source_symbols"] =
        serde_json::json!(["__OpenMinisMissingPinnedSymbol__"]);

    let err = verify_openminis_ui_migration_manifest_value(&root, &manifest)
        .expect_err("missing pinned source symbol must fail");
    assert!(
        err.contains("must resolve as exactly one declaration at pinned commit"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_manifest_rejects_ci_pinned_ref_drift() {
    let root = test_repo_root("openminis-ci-ref-drift");
    fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
    fs::write(
        root.join(".github/workflows/ci.yml"),
        "jobs:\n  gate:\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          repository: OpenMinis/OpenMinis\n          ref: wrong\n          path: external/OpenMinis\n      - uses: swift-actions/setup-swift@v2\n      - run: make ci\n",
    )
    .expect("write ci");

    let err = verify_openminis_ui_ci_checkout(&root, "9cf3a855fecd27bb5735b84cacbd56852a3ab8dd")
        .expect_err("CI ref drift must fail");
    assert!(err.contains("ref must match manifest commit"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_ci_values_from_later_step() {
    let root = test_repo_root("openminis-ci-later-step-bypass");
    fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
    fs::write(
        root.join(".github/workflows/ci.yml"),
        "jobs:
  gate:
    steps:
      - uses: actions/checkout@v4
        with:
          repository: OpenMinis/OpenMinis
      - name: unrelated
        env:
          ref: 9cf3a855fecd27bb5735b84cacbd56852a3ab8dd
          path: external/OpenMinis
        run: echo unrelated
      - uses: swift-actions/setup-swift@v2
      - run: make ci
",
    )
    .expect("write ci");

    let err = verify_openminis_ui_ci_checkout(&root, "9cf3a855fecd27bb5735b84cacbd56852a3ab8dd")
        .expect_err("later-step ref/path must not satisfy checkout step");
    assert!(err.contains("ref must match manifest commit"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_ci_pinned_path_drift() {
    let root = test_repo_root("openminis-ci-path-drift");
    fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
    fs::write(
        root.join(".github/workflows/ci.yml"),
        "jobs:\n  gate:\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          repository: OpenMinis/OpenMinis\n          ref: 9cf3a855fecd27bb5735b84cacbd56852a3ab8dd\n          path: wrong/OpenMinis\n      - uses: swift-actions/setup-swift@v2\n      - run: make ci\n",
    )
    .expect("write ci");

    let err = verify_openminis_ui_ci_checkout(&root, "9cf3a855fecd27bb5735b84cacbd56852a3ab8dd")
        .expect_err("CI path drift must fail");
    assert!(err.contains("path must be `external/OpenMinis`"), "{err}");
}

#[test]
fn openminis_ui_migration_ci_binds_checkout_to_executable_gate_job() {
    let commit = "9cf3a855fecd27bb5735b84cacbd56852a3ab8dd";
    let root = test_repo_root("openminis-ci-same-gate-job");
    fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
    fs::write(
        root.join(".github/workflows/ci.yml"),
        format!(
            "jobs:\n  gate:\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          repository: OpenMinis/OpenMinis\n          ref: {commit}\n          path: external/OpenMinis\n      - uses: swift-actions/setup-swift@v2\n      - run: make ci\n"
        ),
    )
    .expect("write ci");
    verify_openminis_ui_ci_checkout(&root, commit)
        .expect("same executable gate job checkout must pass");

    let root = test_repo_root("openminis-ci-metadata-forgery");
    fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
    fs::write(
        root.join(".github/workflows/ci.yml"),
        format!(
            "metadata:\n  uses: actions/checkout@v4\n  with:\n    repository: OpenMinis/OpenMinis\n    ref: {commit}\n    path: external/OpenMinis\njobs:\n  gate:\n    steps:\n      - run: make ci\n"
        ),
    )
    .expect("write ci");
    let err = verify_openminis_ui_ci_checkout(&root, commit)
        .expect_err("checkout-shaped metadata must not count");
    assert!(err.contains("gate job `gate`"), "{err}");

    let root = test_repo_root("openminis-ci-cross-job-forgery");
    fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
    fs::write(
        root.join(".github/workflows/ci.yml"),
        format!(
            "jobs:\n  source:\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          repository: OpenMinis/OpenMinis\n          ref: {commit}\n          path: external/OpenMinis\n  gate:\n    steps:\n      - uses: swift-actions/setup-swift@v2\n      - run: make ci\n"
        ),
    )
    .expect("write ci");
    let err = verify_openminis_ui_ci_checkout(&root, commit)
        .expect_err("checkout in a different job must not count");
    assert!(err.contains("gate job `gate`"), "{err}");
}

#[test]
fn openminis_ui_migration_rejects_unprovisioned_release_gate() {
    let commit = "9cf3a855fecd27bb5735b84cacbd56852a3ab8dd";
    let root = test_repo_root("openminis-release-unprovisioned");
    fs::create_dir_all(root.join(".github/workflows")).expect("create workflows");
    fs::write(
        root.join(".github/workflows/ci.yml"),
        format!(
            "jobs:\n  gate:\n    steps:\n      - uses: actions/checkout@v4\n        with:\n          repository: OpenMinis/OpenMinis\n          ref: {commit}\n          path: external/OpenMinis\n      - uses: swift-actions/setup-swift@v2\n      - run: make ci\n"
        ),
    )
    .expect("write ci");
    fs::write(
        root.join(".github/workflows/release.yml"),
        "jobs:\n  release:\n    steps:\n      - run: make ci\n",
    )
    .expect("write release");

    let err = verify_openminis_ui_ci_checkout(&root, commit)
        .expect_err("every workflow executing make ci must provision gate prerequisites");
    assert!(err.contains("release.yml"), "{err}");
    assert!(err.contains("OpenMinis/OpenMinis"), "{err}");
}

#[test]
fn openminis_ui_migration_rejects_malformed_human_registry_rows() {
    let fixtures = [
        (
            "edge",
            "## Entrypoint And Registered Paths\n- entrypoint_node_id: `foundation.root`\n### Forward Edges\n| edge_id | from_node_id | to_node_id | semantic |\n| --- | --- | --- | --- |\n| edge | from | to |\n### Return Paths\n| from_node_id | to_node_id | semantic |\n| --- | --- | --- |\n| from | to | back |\n",
        ),
        (
            "return",
            "## Entrypoint And Registered Paths\n- entrypoint_node_id: `foundation.root`\n### Forward Edges\n| edge_id | from_node_id | to_node_id | semantic |\n| --- | --- | --- | --- |\n| edge | from | to | forward |\n### Return Paths\n| from_node_id | to_node_id | semantic |\n| --- | --- | --- |\n| from | to |\n",
        ),
        (
            "unknown",
            "## Entrypoint And Registered Paths\n- entrypoint_node_id: `foundation.root`\n### Forward Edges\n| edge_id | from_node_id | to_node_id | semantic |\n| --- | --- | --- | --- |\n| edge | from | to | forward |\n### Unknown Registry\n| from | to | unexplained |\n### Return Paths\n| from_node_id | to_node_id | semantic |\n| --- | --- | --- |\n| from | to | back |\n",
        ),
    ];
    for (name, markdown) in fixtures {
        let err = parse_openminis_ui_registered_paths(markdown)
            .expect_err("malformed registry row must fail");
        assert!(err.contains("malformed"), "{name}: {err}");
    }
}

#[test]
fn openminis_ui_migration_manifest_rejects_pinned_source_head_drift() {
    let root = test_repo_root("openminis-head-drift");
    let repository = root.join("external/OpenMinis");
    fs::create_dir_all(repository.join("src/ios/Views")).expect("create source repo");
    run_test_git(&repository, &["init"]);
    run_test_git(
        &repository,
        &["config", "user.email", "xtask@example.invalid"],
    );
    run_test_git(&repository, &["config", "user.name", "xtask"]);
    fs::write(
        repository.join("src/ios/Views/ContentView.swift"),
        "struct ContentView {}\n",
    )
    .expect("write first source");
    run_test_git(&repository, &["add", "."]);
    run_test_git(&repository, &["commit", "-m", "pinned"]);
    let pinned = git_stdout(&repository, &["rev-parse", "HEAD"]).expect("pinned commit");
    fs::write(repository.join("README.md"), "new head\n").expect("write second source");
    run_test_git(&repository, &["add", "."]);
    run_test_git(&repository, &["commit", "-m", "head"]);
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

    let err = verify_openminis_ui_pinned_source(
        &root,
        source_repository.as_object().expect("source repository"),
        &[],
    )
    .expect_err("pinned source HEAD drift must fail");
    assert!(err.contains("pinned source HEAD drift"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_substring_only_mainline_symbol() {
    let (root, node) = write_openminis_binding_fixture("substring", "substring", "bound", "bound");
    let mut node = node;
    node["target_symbols"] = serde_json::json!(["exactSymbol"]);

    let err = verify_openminis_ui_target_bindings(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        "ui_projection.render",
    )
    .expect_err("substring-only mainline symbol must fail");
    assert!(err.contains("exact declaration file"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_binds_symbol_to_its_exact_declaration_file() {
    let (root, node) =
        write_openminis_binding_fixture("exact-declaration-file", "exact", "bound", "bound");
    verify_openminis_ui_target_bindings(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        "ui_projection.render",
    )
    .expect("matching declaration and mainline files must pass");

    fs::write(root.join("src/other.js"), "function unrelated() {}\n").expect("write other target");
    let mut node = node;
    node["target_paths"] = serde_json::json!(["src/target.js", "src/other.js"]);
    let mainline_path = root.join("docs/mainline-calls/test.json");
    let mut mainline: Value =
        serde_json::from_str(&fs::read_to_string(&mainline_path).expect("read mainline"))
            .expect("parse mainline");
    mainline["call_table"][0]["file_path"] = Value::String("src/other.js".to_owned());
    fs::write(
        &mainline_path,
        serde_json::to_vec(&mainline).expect("encode mainline"),
    )
    .expect("write mismatched mainline");

    let err = verify_openminis_ui_target_bindings(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        "ui_projection.render",
    )
    .expect_err("mainline row in another valid target file must fail");
    assert!(err.contains("exact declaration file"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_pending_mainline_or_resource_binding() {
    let (root, node) = write_openminis_binding_fixture("pending-row", "exact", "pending", "bound");
    let err = verify_openminis_ui_target_bindings(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        "ui_projection.render",
    )
    .expect_err("pending mainline row must fail");
    assert!(err.contains("exact declaration file"), "{err}");

    let (root, node) =
        write_openminis_binding_fixture("pending-operation", "exact", "bound", "pending");
    let err = verify_openminis_ui_target_bindings(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        "ui_projection.render",
    )
    .expect_err("pending resource operation must fail");
    assert!(err.contains("is not bound in the resource map"), "{err}");
}

#[test]
fn openminis_ui_migration_manifest_rejects_external_target_or_mainline_path() {
    let (root, mut node) =
        write_openminis_binding_fixture("absolute-target", "exact", "bound", "bound");
    node["target_paths"] = serde_json::json!(["/tmp"]);
    let err = verify_openminis_ui_target_bindings(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        "ui_projection.render",
    )
    .expect_err("absolute target path must fail");
    assert!(err.contains("repository-relative path"), "{err}");

    let (root, mut node) =
        write_openminis_binding_fixture("escaping-target", "exact", "bound", "bound");
    node["target_paths"] = serde_json::json!(["../outside"]);
    let err = verify_openminis_ui_target_bindings(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        "ui_projection.render",
    )
    .expect_err("escaping target path must fail");
    assert!(err.contains("repository-relative path"), "{err}");

    let (root, mut node) =
        write_openminis_binding_fixture("absolute-mainline", "exact", "bound", "bound");
    node["mainline_call_docs"] = serde_json::json!(["/tmp"]);
    let err = verify_openminis_ui_target_bindings(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        "ui_projection.render",
    )
    .expect_err("absolute mainline path must fail");
    assert!(err.contains("repository-relative path"), "{err}");
}

#[cfg(unix)]
#[test]
fn openminis_ui_migration_manifest_rejects_symlinked_target_or_mainline_path() {
    use std::os::unix::fs::symlink;

    let (root, mut node) =
        write_openminis_binding_fixture("symlink-target", "exact", "bound", "bound");
    symlink(root.join("src"), root.join("src-alias")).expect("create target symlink");
    node["target_paths"] = serde_json::json!(["src-alias/target.js"]);
    let err = verify_openminis_ui_target_bindings(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        "ui_projection.render",
    )
    .expect_err("symlinked target path must fail");
    assert!(err.contains("symbolic link"), "{err}");

    let (root, mut node) =
        write_openminis_binding_fixture("symlink-mainline", "exact", "bound", "bound");
    symlink(
        root.join("docs/mainline-calls/test.json"),
        root.join("docs/mainline-calls/alias.json"),
    )
    .expect("create mainline symlink");
    node["mainline_call_docs"] = serde_json::json!(["docs/mainline-calls/alias.json"]);
    let err = verify_openminis_ui_target_bindings(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        "ui_projection.render",
    )
    .expect_err("symlinked mainline path must fail");
    assert!(err.contains("symbolic link"), "{err}");
}
