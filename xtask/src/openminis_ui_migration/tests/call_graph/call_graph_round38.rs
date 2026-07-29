use super::*;

#[test]
fn openminis_ui_migration_call_graph_resolves_callable_local_associated_calls() {
    let root = test_repo_root("call-graph-local-associated-call");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn target() {}
fn host() {
    struct Local;
    impl Local {
        fn invoke() { target(); }
    }
    Local::invoke();
}
fn main() {}
"#,
    )
    .expect("local associated-call source");

    let graph = discover_rust_call_graph(&root).expect("discover local associated-call graph");
    assert_eq!(
        graph.callers.get("crate::host::Local::invoke"),
        Some(&BTreeSet::from(["crate::host".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_rejects_active_nested_impls() {
    let root = test_repo_root("call-graph-active-nested-impl");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn target() {}
fn host() {
    if true {
        struct Local;
        impl Local {
            fn invoke() { target(); }
        }
    }
}
fn main() {}
"#,
    )
    .expect("active nested-impl source");

    let err = discover_rust_call_graph(&root).expect_err("active nested impl must fail closed");
    assert!(err.contains("nested impl"), "{err}");

    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn target() {}
fn host() {
    if true {
        struct Local;
        #[cfg(any())]
        impl Local {
            fn invoke() { target(); }
        }
    }
}
fn main() {}
"#,
    )
    .expect("cfg-disabled nested-impl source");
    let graph = discover_rust_call_graph(&root).expect("disabled nested impl must stay absent");
    assert_eq!(graph.callers.get("crate::target"), Some(&BTreeSet::new()));
}

#[test]
fn openminis_ui_migration_call_graph_owns_block_local_initializers() {
    let root = test_repo_root("call-graph-block-local-initializers");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
const fn target() {}
fn host() {
    const DIRECT: () = target();
    if true {
        static NESTED: () = target();
    }
}
fn main() {}
"#,
    )
    .expect("block-local initializer source");

    let graph = discover_rust_call_graph(&root).expect("discover block-local initializer graph");
    assert_eq!(
        graph.callers.get("crate::target"),
        Some(&BTreeSet::from([
            "crate::host::DIRECT::__initializer".to_owned(),
            "crate::host::__block_item_0::NESTED::__initializer".to_owned(),
        ]))
    );
}
