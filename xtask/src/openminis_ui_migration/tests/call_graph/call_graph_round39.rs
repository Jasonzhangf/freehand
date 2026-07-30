use super::*;

#[test]
fn openminis_ui_migration_call_graph_resolves_associated_initializer_self_calls() {
    let root = test_repo_root("call-graph-associated-initializer-self-call");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker {
    const fn helper() -> usize { 1 }
    const VALUE: usize = Self::helper();
}
fn main() {}
"#,
    )
    .expect("associated initializer source");

    let graph = discover_rust_call_graph(&root).expect("discover associated initializer graph");
    assert_eq!(
        graph.callers.get("crate::Worker::helper"),
        Some(&BTreeSet::from([
            "crate::Worker::VALUE::__initializer".to_owned()
        ]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_resolves_local_initializer_associated_calls() {
    let root = test_repo_root("call-graph-local-initializer-associated-call");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn host() {
    struct Local;
    impl Local {
        const fn invoke() -> usize { 1 }
    }
    const VALUE: usize = Local::invoke();
    let _ = VALUE;
}
fn main() {}
"#,
    )
    .expect("local initializer source");

    let graph = discover_rust_call_graph(&root).expect("discover local initializer graph");
    assert_eq!(
        graph.callers.get("crate::host::Local::invoke"),
        Some(&BTreeSet::from([
            "crate::host::VALUE::__initializer".to_owned()
        ]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_prefers_callable_local_type_bindings() {
    let root = test_repo_root("call-graph-callable-local-type-shadow");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Local;
impl Local {
    fn invoke() {}
}
fn host() {
    struct Local;
    impl Local {
        fn invoke() {}
    }
    Local::invoke();
}
fn main() {}
"#,
    )
    .expect("callable-local type shadow source");

    let graph = discover_rust_call_graph(&root).expect("discover local type shadow graph");
    assert_eq!(
        graph.callers.get("crate::host::Local::invoke"),
        Some(&BTreeSet::from(["crate::host".to_owned()]))
    );
    assert_eq!(
        graph.callers.get("crate::Local::invoke"),
        Some(&BTreeSet::new())
    );
}
