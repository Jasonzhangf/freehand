use super::*;

#[test]
fn openminis_ui_migration_call_graph_filters_cfg_exclusive_block_imports() {
    let root = test_repo_root("call-graph-cfg-block-imports");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod prod { pub fn run() {} }
mod checks { pub fn run() {} }

#[cfg(not(test))]
fn production_caller() {
    #[cfg(not(test))]
    use crate::prod::run as selected;
    #[cfg(test)]
    use crate::checks::run as selected;
    selected();
}

#[cfg(test)]
fn test_caller() {
    #[cfg(not(test))]
    use crate::prod::run as selected;
    #[cfg(test)]
    use crate::checks::run as selected;
    selected();
}

fn main() {}
"#,
    )
    .expect("cfg block import source");

    let graph = discover_rust_call_graph(&root).expect("discover cfg-filtered block imports");
    assert_eq!(
        graph.callers.get("crate::prod::run"),
        Some(&BTreeSet::from(["crate::production_caller".to_owned()]))
    );
    assert_eq!(
        graph.callers.get("crate::checks::run"),
        Some(&BTreeSet::from(["crate::test_caller".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_ignores_cfg_disabled_block_value_shadow() {
    let root = test_repo_root("call-graph-cfg-block-value-shadow");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn target() {}

#[cfg(not(test))]
fn production_caller() {
    #[cfg(test)]
    const target: fn() = || {};
    target();
}

fn main() {}
"#,
    )
    .expect("cfg block value source");

    let graph = discover_rust_call_graph(&root).expect("discover active block value scope");
    assert_eq!(
        graph.callers.get("crate::target"),
        Some(&BTreeSet::from(["crate::production_caller".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_preserves_enum_discriminant_initializer() {
    let root = test_repo_root("call-graph-enum-discriminant");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
const fn target() -> isize { 1 }

enum State {
    Ready = target(),
}

fn main() {}
"#,
    )
    .expect("enum discriminant source");

    let graph = discover_rust_call_graph(&root).expect("discover enum discriminant graph");
    assert_eq!(
        graph.callers.get("crate::target"),
        Some(&BTreeSet::from([
            "crate::State::Ready::__discriminant_initializer".to_owned()
        ]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_rejects_cfg_disabled_enum_discriminant() {
    let root = test_repo_root("call-graph-disabled-enum-discriminant");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
const fn target() -> isize { 1 }

enum State {
    #[cfg(any())]
    Ready = target(),
}

fn main() {}
"#,
    )
    .expect("disabled enum discriminant source");

    let err = discover_rust_call_graph(&root)
        .expect_err("cfg-disabled discriminant must not enter call truth");
    assert!(err.contains("disabled by cfg"), "{err}");
}
