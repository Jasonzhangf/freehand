use super::*;

#[test]
fn openminis_ui_migration_call_graph_resolves_function_aliases() {
    let root = test_repo_root("call-graph-function-alias");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod helpers { pub fn helper() {} }
use crate::helpers::helper as invoke;
fn caller() { invoke(); }
fn main() {}
"#,
    )
    .expect("function alias fixture");

    let graph = discover_rust_call_graph(&root).expect("discover function alias graph");
    assert_eq!(
        graph.callers.get("crate::helpers::helper"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_scopes_nested_block_imports() {
    let root = test_repo_root("call-graph-nested-block-import");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod outer { pub fn work() {} }
mod inner { pub fn work() {} }
fn caller() {
    use crate::outer::work;
    work();
    {
        use crate::inner::work;
        work();
    }
    work();
}
fn main() {}
"#,
    )
    .expect("nested block import fixture");

    let graph = discover_rust_call_graph(&root).expect("discover nested import graph");
    assert_eq!(
        graph.callers.get("crate::outer::work"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );
    assert_eq!(
        graph.callers.get("crate::inner::work"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_respects_block_item_callable_shadowing() {
    let root = test_repo_root("call-graph-block-item-shadow");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn const_handler() {}
fn static_handler() {}
fn caller() {
    const const_handler: fn() = || {};
    static static_handler: fn() = || {};
    const_handler();
    static_handler();
}
fn main() {}
"#,
    )
    .expect("block item shadow fixture");

    let graph = discover_rust_call_graph(&root).expect("discover block item shadow graph");
    assert_eq!(
        graph.callers.get("crate::const_handler"),
        Some(&BTreeSet::new())
    );
    assert_eq!(
        graph.callers.get("crate::static_handler"),
        Some(&BTreeSet::new())
    );
}

#[test]
fn openminis_ui_migration_call_graph_preserves_destructured_loop_field_type() {
    let root = test_repo_root("call-graph-loop-field-type");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Child;
impl Child { fn run(&self) {} }
struct Worker { child: Child }
impl Worker { fn run(&self) {} }
fn caller(workers: Vec<Worker>) {
    for Worker { child } in workers {
        child.run();
    }
}
fn main() {}
"#,
    )
    .expect("destructured loop field fixture");

    let graph = discover_rust_call_graph(&root).expect("discover loop field graph");
    assert_eq!(
        graph.callers.get("crate::Child::run"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::new())
    );
}
