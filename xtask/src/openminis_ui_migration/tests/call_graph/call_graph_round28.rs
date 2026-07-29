use super::*;

#[test]
fn openminis_ui_migration_call_graph_omits_cfg_disabled_body_calls() {
    let root = test_repo_root("call-graph-body-cfg");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn target() {}
fn caller() {
    #[cfg(any())]
    target();
}
fn main() {}
"#,
    )
    .expect("body cfg fixture");

    let graph = discover_rust_call_graph(&root).expect("discover body cfg graph");
    assert_eq!(graph.callers.get("crate::target"), Some(&BTreeSet::new()));
}

#[test]
fn openminis_ui_migration_call_graph_resolves_nested_glob_imports() {
    let root = test_repo_root("call-graph-nested-glob");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod helper { pub fn target() {} }
fn caller() {
    {
        use crate::helper::*;
        target();
    }
}
fn main() {}
"#,
    )
    .expect("nested glob fixture");

    let graph = discover_rust_call_graph(&root).expect("discover nested glob graph");
    assert_eq!(
        graph.callers.get("crate::helper::target"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_rejects_ambiguous_nested_globs() {
    let root = test_repo_root("call-graph-ambiguous-nested-glob");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod first { pub fn target() {} }
mod second { pub fn target() {} }
fn caller() {
    use crate::first::*;
    use crate::second::*;
    target();
}
fn main() {}
"#,
    )
    .expect("ambiguous nested glob fixture");

    let err = discover_rust_call_graph(&root).expect_err("ambiguous glob call must fail");
    assert!(err.contains("ambiguous lexical glob call"), "{err}");
}

#[test]
fn openminis_ui_migration_call_graph_accepts_production_only_cfg() {
    let root = test_repo_root("call-graph-production-only-cfg");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn target() {}
#[cfg(not(test))]
fn production() { target(); }
#[cfg(test)]
fn test_only() { target(); }
fn main() {}
"#,
    )
    .expect("production-only cfg fixture");

    let graph = discover_rust_call_graph(&root).expect("discover split cfg graph");
    assert_eq!(
        graph.callers.get("crate::target"),
        Some(&BTreeSet::from([
            "crate::production".to_owned(),
            "crate::test_only".to_owned(),
        ]))
    );
    assert!(!graph.test_symbols.contains("crate::production"));
    assert!(graph.test_symbols.contains("crate::test_only"));
}
