use super::*;

#[test]
fn openminis_ui_migration_call_graph_preserves_module_initializer_callers() {
    let root = test_repo_root("call-graph-module-initializers");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
const fn const_target() -> usize { 1 }
fn runtime_target() {}

const VALUE: usize = const_target();
static HANDLER: fn() = || runtime_target();

fn main() {}
"#,
    )
    .expect("initializer source");

    let graph = discover_rust_call_graph(&root).expect("discover initializer graph");
    assert_eq!(
        graph.callers.get("crate::const_target"),
        Some(&BTreeSet::from(["crate::VALUE::__initializer".to_owned()]))
    );
    assert_eq!(
        graph.callers.get("crate::runtime_target"),
        Some(&BTreeSet::from(
            ["crate::HANDLER::__initializer".to_owned()]
        ))
    );
}

#[test]
fn openminis_ui_migration_call_graph_rejects_cfg_disabled_module_initializer() {
    let root = test_repo_root("call-graph-disabled-module-initializer");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        "#[cfg(any())]\nstatic DISABLED: fn() = || target();\nfn target() {}\nfn main() {}\n",
    )
    .expect("disabled initializer source");

    let err = discover_rust_call_graph(&root).expect_err("disabled initializer must fail closed");
    assert!(err.contains("disabled by cfg"), "{err}");
}

#[test]
fn openminis_ui_migration_call_graph_resolves_local_deref_target_methods() {
    let root = test_repo_root("call-graph-local-deref-target");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
use std::ops::Deref;

struct Local;
impl Local {
    fn local_method(&self) {}
}

struct Wrapper(Local);
impl Deref for Wrapper {
    type Target = Local;
    fn deref(&self) -> &Self::Target { &self.0 }
}

fn invoke(wrapper: &Wrapper) {
    wrapper.local_method();
}

fn main() {}
"#,
    )
    .expect("deref source");

    let graph = discover_rust_call_graph(&root).expect("discover deref graph");
    assert_eq!(
        graph.callers.get("crate::Local::local_method"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_keeps_unrelated_external_trait_method_external() {
    let root = test_repo_root("call-graph-external-trait-method");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
use std::ops::Deref;

struct Local;
impl Local {
    fn local_method(&self) {}
}

struct Wrapper(Local);
impl Deref for Wrapper {
    type Target = Local;
    fn deref(&self) -> &Self::Target { &self.0 }
}

fn invoke(wrapper: &Wrapper) {
    wrapper.count();
}

fn main() {}
"#,
    )
    .expect("external trait source");

    let graph = discover_rust_call_graph(&root).expect("external method remains external");
    assert_eq!(
        graph.callers.get("crate::Local::local_method"),
        Some(&BTreeSet::new())
    );
}

#[test]
fn openminis_ui_migration_call_graph_preserves_associated_const_initializers() {
    let root = test_repo_root("call-graph-associated-const-initializers");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
const fn impl_target() -> usize { 1 }
const fn trait_target() -> usize { 2 }

struct Worker;
impl Worker {
    const VALUE: usize = impl_target();
}

trait Defaults {
    const VALUE: usize = trait_target();
}

fn main() {}
"#,
    )
    .expect("associated initializer source");

    let graph = discover_rust_call_graph(&root).expect("discover associated initializer graph");
    assert_eq!(
        graph.callers.get("crate::impl_target"),
        Some(&BTreeSet::from([
            "crate::Worker::VALUE::__initializer".to_owned()
        ]))
    );
    assert_eq!(
        graph.callers.get("crate::trait_target"),
        Some(&BTreeSet::from([
            "crate::Defaults::VALUE::__initializer".to_owned()
        ]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_rejects_cfg_disabled_associated_initializer() {
    let root = test_repo_root("call-graph-disabled-associated-initializer");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
const fn target() -> usize { 1 }
struct Worker;
impl Worker {
    #[cfg(any())]
    const VALUE: usize = target();
}
fn main() {}
"#,
    )
    .expect("disabled associated initializer source");

    let err =
        discover_rust_call_graph(&root).expect_err("disabled associated initializer must fail");
    assert!(err.contains("disabled by cfg"), "{err}");
}
