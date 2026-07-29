use super::*;

#[test]
fn openminis_ui_migration_call_graph_preserves_unwrapped_local_receiver_type() {
    let root = test_repo_root("call-graph-unwrapped-local-receiver");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) {} }
fn make_worker() -> Result<Worker, ()> { Ok(Worker) }
fn caller() { make_worker().unwrap().run(); }
fn main() {}
"#,
    )
    .expect("wrapped receiver fixture");

    let graph = discover_rust_call_graph(&root).expect("discover wrapped receiver graph");
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_edges_stop_at_external_callee_boundary() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let mainline: Value = serde_json::from_str(
        &fs::read_to_string(root.join("docs/mainline-calls/foundation.workspace.json"))
            .expect("read foundation mainline"),
    )
    .expect("parse foundation mainline");
    for row in mainline["call_table"].as_array().expect("call table") {
        if row["graph_id"] != "openminis_ui_migration" {
            continue;
        }
        let caller = row["caller"].as_str().expect("caller");
        let callee = row["callee"].as_str().expect("callee");
        assert!(
            caller.starts_with("crate::openminis_ui_migration")
                || callee.starts_with("crate::openminis_ui_migration"),
            "outside-to-outside edge leaked into migration truth: {caller} -> {callee}"
        );
    }
}

#[test]
fn openminis_ui_migration_call_graph_resolves_qualified_reexports() {
    let root = test_repo_root("call-graph-qualified-reexport");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod inner { pub fn target() {} }
mod api { pub use crate::inner::target; }
fn caller() { api::target(); }
fn main() {}
"#,
    )
    .expect("qualified reexport fixture");

    let graph = discover_rust_call_graph(&root).expect("discover qualified reexport graph");
    assert_eq!(
        graph.callers.get("crate::inner::target"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_preserves_parenthesized_direct_callee() {
    let root = test_repo_root("call-graph-parenthesized-direct-callee");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn target() {}
fn caller() { (target)(); }
fn main() {}
"#,
    )
    .expect("parenthesized direct callee fixture");

    let graph = discover_rust_call_graph(&root).expect("discover parenthesized direct call graph");
    assert_eq!(
        graph.callers.get("crate::target"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_updates_reassigned_receiver_type() {
    let root = test_repo_root("call-graph-reassigned-receiver");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) {} }
fn caller(mut value: String) { value = Worker; value.run(); }
fn main() {}
"#,
    )
    .expect("reassigned receiver fixture");

    let graph = discover_rust_call_graph(&root).expect("discover reassigned receiver graph");
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );

    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) {} }
fn caller(mut value: Worker) { value = String::new(); value.run(); }
fn main() {}
"#,
    )
    .expect("external reassignment fixture");
    let graph = discover_rust_call_graph(&root).expect("discover external reassignment graph");
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::new()),
        "an external reassignment must invalidate the previous local receiver type"
    );
}

#[test]
fn openminis_ui_migration_call_graph_preserves_container_unwrap_receiver_type() {
    let root = test_repo_root("call-graph-container-unwrapped-receiver");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) {} }
fn caller(mut workers: Vec<Worker>) { workers.pop().unwrap().run(); }
fn main() {}
"#,
    )
    .expect("container unwrapped receiver fixture");

    let graph = discover_rust_call_graph(&root).expect("discover container receiver graph");
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );
}
