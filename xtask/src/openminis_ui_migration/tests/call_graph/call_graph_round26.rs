use super::*;

#[test]
fn openminis_ui_migration_call_graph_preserves_recursive_edges() {
    let root = test_repo_root("call-graph-recursive-edges");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn recurse(depth: usize) {
    if depth > 0 { recurse(depth - 1); }
}
struct Worker;
impl Worker {
    fn run(&self, depth: usize) {
        if depth > 0 { self.run(depth - 1); }
    }
}
fn main() {
    recurse(1);
    let worker = Worker;
    worker.run(1);
}
"#,
    )
    .expect("recursive edge fixture");

    let graph = discover_rust_call_graph(&root).expect("discover recursive graph");
    assert_eq!(
        graph.callers.get("crate::recurse"),
        Some(&BTreeSet::from([
            "crate::main".to_owned(),
            "crate::recurse".to_owned()
        ]))
    );
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::from([
            "crate::Worker::run".to_owned(),
            "crate::main".to_owned()
        ]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_respects_callable_value_shadowing() {
    let root = test_repo_root("call-graph-callable-value-shadow");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn callback() {}
fn local_callback() {}
fn invoke(callback: fn()) {
    callback();
    let local_callback = callback;
    local_callback();
}
fn main() { invoke(|| {}); }
"#,
    )
    .expect("callable shadow fixture");

    let graph = discover_rust_call_graph(&root).expect("discover callable shadow graph");
    assert_eq!(graph.callers.get("crate::callback"), Some(&BTreeSet::new()));
    assert_eq!(
        graph.callers.get("crate::local_callback"),
        Some(&BTreeSet::new())
    );
}

#[test]
fn openminis_ui_migration_call_graph_inherits_external_module_test_identity() {
    let root = test_repo_root("call-graph-external-test-module");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        "fn target() {}\n#[cfg(test)] mod checks;\nfn main() {}\n",
    )
    .expect("external test module root");
    fs::write(
        root.join("xtask/src/checks.rs"),
        "mod nested;\npub fn helper() { super::target(); }\n",
    )
    .expect("external test module body");
    fs::create_dir_all(root.join("xtask/src/checks")).expect("nested test module dir");
    fs::write(
        root.join("xtask/src/checks/nested.rs"),
        "pub fn nested_helper() { super::super::target(); }\n",
    )
    .expect("nested external test module body");

    let graph = discover_rust_call_graph(&root).expect("discover external test module graph");
    assert!(graph.test_symbols.contains("crate::checks::helper"));
    assert!(
        graph
            .test_symbols
            .contains("crate::checks::nested::nested_helper")
    );
    assert_eq!(
        graph.callers.get("crate::target"),
        Some(&BTreeSet::from([
            "crate::checks::helper".to_owned(),
            "crate::checks::nested::nested_helper".to_owned()
        ]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_scopes_conditional_let_bindings() {
    for (case, body) in [
        (
            "if-let-chain",
            "if let Some(value) = other && value.len() > 0 { value.len() } else { 0 }",
        ),
        (
            "while-let",
            "let mut total = 0; let mut iter = other.into_iter(); while let Some(value) = iter.next() { total += value.len(); } total",
        ),
    ] {
        let root = test_repo_root(&format!("call-graph-{case}-shadow"));
        fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
        fs::write(
            root.join("xtask/src/main.rs"),
            format!(
                "struct Worker;\nimpl Worker {{ fn len(&self) -> usize {{ 0 }} }}\nfn invoke(value: Worker, other: Option<String>) -> usize {{ {body} }}\nfn main() {{}}\n"
            ),
        )
        .expect("conditional let shadow fixture");

        let err = discover_rust_call_graph(&root)
            .expect_err("conditional binder must not retain the outer receiver");
        assert!(
            err.contains("cannot resolve potentially local method call `len`"),
            "{case}: {err}"
        );
    }
}

#[test]
fn openminis_ui_migration_call_graph_clears_for_binder_iterable_state() {
    let root = test_repo_root("call-graph-for-iterable-shadow");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn len(&self) -> usize { 0 } }
fn invoke(items: Vec<Worker>, groups: Vec<Vec<String>>) -> usize {
    let mut total = 0;
    for items in groups {
        for item in items { total += item.len(); }
    }
    total
}
fn main() {}
"#,
    )
    .expect("for iterable shadow fixture");

    let graph = discover_rust_call_graph(&root).expect("discover loop iterable shadow graph");
    assert_eq!(
        graph.callers.get("crate::Worker::len"),
        Some(&BTreeSet::new())
    );
}

#[test]
fn openminis_ui_migration_call_graph_tracks_typed_closure_iterables() {
    let root = test_repo_root("call-graph-typed-closure-iterable");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) {} }
fn invoke() {
    let consume = |items: Vec<Worker>| {
        for item in items { item.run(); }
    };
    consume(Vec::new());
}
fn main() {}
"#,
    )
    .expect("typed closure iterable fixture");

    let graph = discover_rust_call_graph(&root).expect("discover typed closure iterable graph");
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}
