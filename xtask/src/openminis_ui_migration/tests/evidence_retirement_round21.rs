use super::super::*;
use super::common::*;

#[test]
fn openminis_ui_migration_legacy_retirement_scans_binary_asset_roots() {
    let (root, node, gates) = write_openminis_retirement_fixture("binary-assets", false);
    fs::write(
        root.join("scan/launcher.png"),
        [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0xff],
    )
    .expect("write binary asset");
    verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect("binary assets must not prevent owner-root retirement scans");

    fs::write(
        root.join("scan/current.js"),
        "legacySymbol remains text-visible",
    )
    .expect("write remaining text identity");
    let err = verify_openminis_ui_legacy_retirement(
        &root,
        "foundation.root",
        node.as_object().expect("node"),
        &gates,
    )
    .expect_err("binary coexistence must not hide legacy text identities");
    assert!(err.contains("legacy symbol still resolves"), "{err}");
}
