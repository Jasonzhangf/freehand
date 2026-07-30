use super::*;

#[test]
fn openminis_ui_migration_call_graph_honors_nested_expression_cfg() {
    let root = test_repo_root("call-graph-nested-expression-cfg");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn target() {}
fn caller() {
    let _ = (
        #[cfg(any())]
        target(),
    );
}
fn main() {}
"#,
    )
    .expect("nested expression cfg fixture");

    let graph = discover_rust_call_graph(&root).expect("discover nested expression cfg graph");
    assert_eq!(graph.callers.get("crate::target"), Some(&BTreeSet::new()));
}

#[test]
fn openminis_ui_migration_call_graph_honors_match_arm_cfg() {
    let root = test_repo_root("call-graph-match-arm-cfg");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn disabled() {}
fn active() {}
fn caller() {
    match () {
        #[cfg(any())]
        _ => disabled(),
        #[cfg(all())]
        _ => active(),
    }
}
fn main() {}
"#,
    )
    .expect("match arm cfg fixture");

    let graph = discover_rust_call_graph(&root).expect("discover match arm cfg graph");
    assert_eq!(graph.callers.get("crate::disabled"), Some(&BTreeSet::new()));
    assert_eq!(
        graph.callers.get("crate::active"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_resolves_module_qualified_glob_paths() {
    let root = test_repo_root("call-graph-module-qualified-glob");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod prelude {
    pub mod nested { pub fn target() {} }
}
use crate::prelude::*;
fn caller() { nested::target(); }
fn main() {}
"#,
    )
    .expect("module qualified glob fixture");

    let graph = discover_rust_call_graph(&root).expect("discover module qualified glob graph");
    assert_eq!(
        graph.callers.get("crate::prelude::nested::target"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_resolves_callable_qualified_glob_paths() {
    let root = test_repo_root("call-graph-callable-qualified-glob");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod prelude {
    pub mod nested { pub fn target() {} }
}
fn caller() {
    use crate::prelude::*;
    nested::target();
}
fn main() {}
"#,
    )
    .expect("callable qualified glob fixture");

    let graph = discover_rust_call_graph(&root).expect("discover callable qualified glob graph");
    assert_eq!(
        graph.callers.get("crate::prelude::nested::target"),
        Some(&BTreeSet::from(["crate::caller".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_rejects_ambiguous_qualified_globs() {
    let root = test_repo_root("call-graph-ambiguous-qualified-glob");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod first { pub mod nested { pub fn target() {} } }
mod second { pub mod nested { pub fn target() {} } }
use crate::first::*;
use crate::second::*;
fn caller() { nested::target(); }
fn main() {}
"#,
    )
    .expect("ambiguous qualified glob fixture");

    let err = discover_rust_call_graph(&root).expect_err("ambiguous qualified glob must fail");
    assert!(err.contains("ambiguous"), "{err}");
}

#[test]
fn openminis_ui_migration_test_repo_root_cleans_up_on_drop() {
    let path = {
        let root = test_repo_root("guard-cleanup");
        let path = root.to_path_buf();
        assert!(
            path.is_dir(),
            "fixture root must exist while guard is alive"
        );
        path
    };
    assert!(
        !path.exists(),
        "fixture root must be removed when guard drops"
    );
}
