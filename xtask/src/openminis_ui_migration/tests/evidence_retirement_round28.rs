use super::super::*;
use super::common::*;

#[cfg(unix)]
#[test]
fn openminis_ui_migration_manifest_rejects_legacy_symlink_escape_or_broken_path() {
    use std::os::unix::fs::symlink;

    let (root, mut node, gates) = write_openminis_retirement_fixture("root-symlink", false);
    symlink(root.join("scan"), root.join("scan-alias")).expect("create scan-root symlink");
    node["legacy_retirement"]["scan_paths"] = serde_json::json!(["scan-alias"]);
    node["legacy_retirement"]["removed_paths"] =
        serde_json::json!(["scan-alias/legacy-removed.js"]);
    let mainline_path = root.join("docs/mainline-calls/app.webui-smoke.json");
    let mut mainline: Value =
        serde_json::from_str(&fs::read_to_string(&mainline_path).expect("read mainline"))
            .expect("parse mainline");
    mainline["legacy_scan_roots"][0]["scan_paths"] = serde_json::json!(["scan-alias"]);
    mainline["legacy_scan_roots"][0]["removed_paths"] =
        serde_json::json!(["scan-alias/legacy-removed.js"]);
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
    .expect_err("symlinked scan root must fail");
    assert!(err.contains("symbolic link"), "{err}");

    let (root, node, gates) = write_openminis_retirement_fixture("nested-symlink", false);
    let outside = test_repo_root("openminis-retirement-outside-symlink");
    fs::write(outside.join("outside.js"), "function outside() {}\n").expect("write outside");
    symlink(&*outside, root.join("scan/escape")).expect("create nested directory symlink");
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("nested scan symlink must fail");
    assert!(err.contains("symbolic link"), "{err}");

    let (root, node, gates) = write_openminis_retirement_fixture("broken-symlink", false);
    symlink(
        root.join("missing-target"),
        root.join("scan/legacy-removed.js"),
    )
    .expect("create broken legacy symlink");
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("broken legacy symlink must still count as present");
    assert!(err.contains("legacy path still exists"), "{err}");
}
