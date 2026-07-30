use super::*;

#[test]
fn openminis_ui_migration_call_graph_keeps_disjunctive_cfg_production_edge() {
    let root = test_repo_root("call-graph-disjunctive-cfg-production");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn target() {}
#[cfg(any(test, unix))]
fn invoke() { target(); }
fn main() {}
"#,
    )
    .expect("disjunctive cfg fixture");

    let graph = discover_rust_call_graph(&root).expect("discover disjunctive cfg graph");
    assert!(!graph.test_symbols.contains("crate::invoke"));
    assert_eq!(
        graph.callers.get("crate::target"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_rejects_match_and_if_let_shadow_edges() {
    for (case, body) in [
        (
            "match",
            "match other { Some(value) => value.len(), None => 0 }",
        ),
        (
            "if-let",
            "if let Some(value) = other { value.len() } else { 0 }",
        ),
    ] {
        let root = test_repo_root(&format!("call-graph-{case}-shadow"));
        fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
        fs::write(
            root.join("xtask/src/main.rs"),
            format!(
                "struct Worker;\nimpl Worker {{ fn len(&self) -> usize {{ 0 }} }}\nfn invoke(value: Worker, other: Option<String>) -> usize {{\n    {body}\n}}\nfn main() {{}}\n"
            ),
        )
        .expect("pattern shadow fixture");

        let err = discover_rust_call_graph(&root)
            .expect_err("pattern binding must not retain the outer receiver");
        assert!(
            err.contains("cannot resolve potentially local method call `len`"),
            "{case}: {err}"
        );
    }
}

#[test]
fn openminis_ui_migration_call_graph_clears_closure_iterable_shadow() {
    let root = test_repo_root("call-graph-closure-iterable-shadow");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn len(&self) -> usize { 0 } }
fn invoke(items: Vec<Worker>) {
    let call = |items| for item in items { let _ = item.len(); };
    call(vec![String::new()]);
}
fn main() {}
"#,
    )
    .expect("closure iterable shadow fixture");

    let err = discover_rust_call_graph(&root)
        .expect_err("closure iterable binding must not retain the outer item type");
    assert!(
        err.contains("cannot resolve potentially local method call `len`"),
        "{err}"
    );
}
