use super::*;

#[test]
fn openminis_ui_migration_call_graph_rejects_untyped_closure_shadow_edge() {
    let root = test_repo_root("call-graph-untyped-closure-shadow");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) {} }
fn invoke() {
    let worker = Worker;
    let values = Vec::from([String::new()]);
    values.iter().for_each(|worker| worker.run());
}
fn main() {}
"#,
    )
    .expect("untyped closure shadow fixture");

    let err = discover_rust_call_graph(&root)
        .expect_err("unknown closure receiver must not reuse the outer local type");
    assert!(
        err.contains("cannot resolve potentially local method call `run`"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_call_graph_visits_shadow_initializer_in_outer_scope() {
    let root = test_repo_root("call-graph-shadow-initializer");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) -> String { String::new() } }
fn invoke() {
    let worker = Worker;
    let worker = worker.run();
    let _ = worker;
}
fn main() {}
"#,
    )
    .expect("shadow initializer fixture");

    let graph = discover_rust_call_graph(&root).expect("discover shadow initializer graph");
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_rejects_unresolved_loop_shadow_edge() {
    let root = test_repo_root("call-graph-loop-shadow");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) {} }
struct Collection;
fn invoke() {
    let item = Worker;
    let values = Collection;
    for item in values { item.run(); }
}
fn main() {}
"#,
    )
    .expect("loop shadow fixture");

    let err = discover_rust_call_graph(&root)
        .expect_err("unknown loop receiver must not reuse the outer local type");
    assert!(
        err.contains("cannot resolve potentially local method call `run`"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_call_graph_resolves_inherited_trait_default() {
    let root = test_repo_root("call-graph-inherited-trait-default");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
trait Runner { fn execute(&self) {} }
impl Runner for Worker {}
fn invoke(worker: &Worker) { worker.execute(); }
fn main() {}
"#,
    )
    .expect("inherited trait default fixture");

    let graph = discover_rust_call_graph(&root).expect("discover inherited trait default graph");
    assert_eq!(
        graph.callers.get("crate::Runner::execute"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}
