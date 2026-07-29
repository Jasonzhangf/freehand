use super::*;

#[test]
fn openminis_ui_migration_call_graph_separates_cfg_projection_imports() {
    let root = test_repo_root("call-graph-cfg-projection-imports");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod prod { pub fn run() {} }
mod checks { pub fn run() {} }

#[cfg(not(test))]
use prod::run;
#[cfg(test)]
use checks::run;

fn production_caller() { run(); }
#[cfg(test)]
fn test_caller() { run(); }
fn main() {}
"#,
    )
    .expect("cfg projection import source");

    let graph = discover_rust_call_graph(&root).expect("discover separate cfg import scopes");
    assert_eq!(
        graph.callers.get("crate::prod::run"),
        Some(&BTreeSet::from(["crate::production_caller".to_owned()]))
    );
    assert_eq!(
        graph.callers.get("crate::checks::run"),
        Some(&BTreeSet::from(["crate::test_caller".to_owned()]))
    );
    assert!(graph.test_symbols.contains("crate::test_caller"));
}

#[test]
fn openminis_ui_migration_call_graph_accepts_cfg_exclusive_same_name_definitions() {
    let root = test_repo_root("call-graph-cfg-exclusive-definitions");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn production_target() {}
fn test_target() {}

#[cfg(not(test))]
fn selected() { production_target(); }
#[cfg(test)]
fn selected() { test_target(); }

fn main() {}
"#,
    )
    .expect("cfg-exclusive definition source");

    let graph = discover_rust_call_graph(&root).expect("cfg-exclusive definitions are valid");
    assert_eq!(
        graph.callers.get("crate::production_target"),
        Some(&BTreeSet::from(["crate::selected".to_owned()]))
    );
    assert_eq!(
        graph.callers.get("crate::test_target"),
        Some(&BTreeSet::new())
    );
    assert!(!graph.test_symbols.contains("crate::selected"));
}
