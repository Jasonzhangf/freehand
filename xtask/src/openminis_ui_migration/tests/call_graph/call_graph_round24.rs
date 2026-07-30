use super::*;

#[test]
fn openminis_ui_migration_call_graph_resolves_callable_local_imports() {
    let root = test_repo_root("call-graph-callable-local-import");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod helpers { pub fn helper() {} }
fn invoke() { use crate::helpers::helper; helper(); }
fn main() {}
"#,
    )
    .expect("callable local import fixture");

    let graph = discover_rust_call_graph(&root).expect("discover callable-local import graph");
    assert_eq!(
        graph.callers.get("crate::helpers::helper"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_preserves_container_item_receiver() {
    let root = test_repo_root("call-graph-container-item");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) {} }
fn invoke(items: Vec<Worker>, slice: &[Worker]) {
    for item in items { item.run(); }
    for item in slice { item.run(); }
}
fn main() {}
"#,
    )
    .expect("container item fixture");

    let graph = discover_rust_call_graph(&root).expect("discover container item graph");
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_clears_destructured_let_bindings() {
    let root = test_repo_root("call-graph-destructured-let");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn len(&self) -> usize { 0 } }
fn invoke(value: Worker, pair: (String, ())) {
    let (value, _) = pair;
    value.len();
}
fn main() {}
"#,
    )
    .expect("destructured let fixture");

    let err = discover_rust_call_graph(&root)
        .expect_err("destructured binding must not retain the outer receiver");
    assert!(
        err.contains("cannot resolve potentially local method call `len`"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_call_graph_marks_nested_cfg_test_predicates() {
    let root = test_repo_root("call-graph-nested-cfg-test");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn helper() {}
#[cfg(all(test, unix))]
mod checks {
    use super::helper;
    fn check() { helper(); }
}
fn main() {}
"#,
    )
    .expect("nested cfg test fixture");

    let graph = discover_rust_call_graph(&root).expect("discover nested cfg test graph");
    assert!(graph.test_symbols.contains("crate::checks::check"));
}

#[test]
fn openminis_ui_migration_call_graph_rejects_unresolved_test_method() {
    let root = test_repo_root("call-graph-unresolved-test-method");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) {} }
#[test]
fn check() {
    let item = Some(Worker).unwrap();
    item.run();
}
fn main() {}
"#,
    )
    .expect("unresolved test method fixture");

    let err = discover_rust_call_graph(&root)
        .expect_err("unresolved test receiver must fail on a local method collision");
    assert!(
        err.contains("cannot resolve potentially local method call `run`"),
        "{err}"
    );
}
