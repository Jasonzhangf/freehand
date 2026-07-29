use super::*;

#[test]
fn openminis_ui_migration_call_graph_filters_cfg_disabled_block_items_before_indexing() {
    let root = test_repo_root("call-graph-cfg-disabled-block-index");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod production { pub fn run() {} }
mod inactive { pub fn run() {} }
use crate::production::run;

fn caller() {
    #[cfg(any())]
    use crate::inactive::run;
    run();
}

fn target() {}
struct Worker;
fn host() {
    #[cfg(any())]
    impl Worker {
        fn phantom() { crate::target(); }
    }
}
"#,
    )
    .expect("cfg-disabled block-item source");

    let graph = discover_rust_call_graph(&root).expect("discover cfg-filtered block-item graph");
    assert_eq!(
        graph.callers.get("crate::production::run"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );
    assert_eq!(
        graph.callers.get("crate::inactive::run"),
        Some(&BTreeSet::new())
    );
    assert_eq!(graph.callers.get("crate::target"), Some(&BTreeSet::new()));
}

#[test]
fn openminis_ui_migration_call_graph_resolves_local_item_methods_in_enclosing_module() {
    let root = test_repo_root("call-graph-local-item-enclosing-module");
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

    trait LocalTrait {
        fn invoke_default() { target(); }
    }
}
"#,
    )
    .expect("local-item method source");

    let graph = discover_rust_call_graph(&root).expect("discover local-item method graph");
    assert_eq!(
        graph.callers.get("crate::target"),
        Some(&BTreeSet::from([
            "crate::host::Local::invoke".to_owned(),
            "crate::host::LocalTrait::invoke_default".to_owned(),
        ]))
    );
}
