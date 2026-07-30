use super::super::*;
use super::common::*;

mod call_graph_round18;
mod call_graph_round23;
mod call_graph_round24;
mod call_graph_round25;
mod call_graph_round26;
mod call_graph_round27;
mod call_graph_round28;
mod call_graph_round29;
mod call_graph_round30;
mod call_graph_round33;
mod call_graph_round35;
mod call_graph_round36;
mod call_graph_round37;
mod call_graph_round38;
mod call_graph_round39;

#[test]
fn openminis_ui_migration_call_graph_accepts_exact_registry() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    verify_openminis_ui_call_graph(&root).expect("exact Rust caller/test graph should pass");
}

#[test]
fn openminis_ui_migration_call_graph_rejects_missing_direct_caller() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let path = root.join("docs/mainline-calls/foundation.workspace.json");
    let mut mainline: Value =
        serde_json::from_str(&fs::read_to_string(path).expect("read mainline"))
            .expect("parse mainline");
    let rows = mainline["shared_functions"]
        .as_array_mut()
        .expect("shared functions");
    let row = rows
        .iter_mut()
        .find(|row| {
            row["symbol"]
                == "crate::openminis_ui_migration::verify_openminis_ui_migration_manifest_value"
        })
        .expect("tracked validator row");
    row["allowed_callers"]
        .as_array_mut()
        .expect("allowed callers")
        .retain(|caller| {
            caller != "crate::openminis_ui_migration::verify_openminis_ui_migration_manifest"
        });

    let err = verify_openminis_ui_call_graph_value(&root, &mainline)
        .expect_err("missing real caller must fail");
    assert!(err.contains("caller drift"), "{err}");

    let mut mainline: Value = serde_json::from_str(
        &fs::read_to_string(root.join("docs/mainline-calls/foundation.workspace.json"))
            .expect("read mainline"),
    )
    .expect("parse mainline");
    mainline["shared_functions"]
        .as_array_mut()
        .expect("shared functions")
        .retain(|row| {
            row["symbol"] != "crate::openminis_ui_migration::support::collect_regular_files"
        });
    let err = verify_openminis_ui_call_graph_value(&root, &mainline)
        .expect_err("missing discovered multi-reference function must fail");
    assert!(err.contains("shared-function registry drift"), "{err}");

    let mut mainline: Value = serde_json::from_str(
        &fs::read_to_string(root.join("docs/mainline-calls/foundation.workspace.json"))
            .expect("read mainline"),
    )
    .expect("parse mainline");
    let edge = mainline["call_table"]
        .as_array_mut()
        .expect("call table")
        .iter_mut()
        .find(|row| {
            row["caller"] == "crate::run_gates_check"
                && row["callee"]
                    == "crate::openminis_ui_migration::verify_openminis_ui_migration_manifest"
        })
        .expect("migration edge");
    edge["caller"] = Value::String("workspace gate plus tests".to_owned());
    let err = verify_openminis_ui_call_graph_value(&root, &mainline)
        .expect_err("prose caller alias must fail exact edge graph");
    assert!(err.contains("module-qualified caller and callee"), "{err}");

    let mut mainline: Value = serde_json::from_str(
        &fs::read_to_string(root.join("docs/mainline-calls/foundation.workspace.json"))
            .expect("read mainline"),
    )
    .expect("parse mainline");
    mainline["call_table"]
        .as_array_mut()
        .expect("call table")
        .retain(|row| {
            row["callee"] != "crate::require_contains"
                || !row["caller"]
                    .as_str()
                    .is_some_and(|caller| caller.starts_with("crate::openminis_ui_migration::"))
        });
    let err = verify_openminis_ui_call_graph_value(&root, &mainline)
        .expect_err("external callee must be source-derived rather than registry-authorized");
    assert!(err.contains("direct call-edge registry drift"), "{err}");
}

#[test]
fn openminis_ui_migration_call_graph_keeps_same_named_modules_distinct() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf();
    let graph = discover_rust_call_graph(&root).expect("parse module/import graph");
    let root_helper = "crate::tests::test_repo_root";
    let migration_helper = "crate::openminis_ui_migration::tests::common::test_repo_root";

    assert_eq!(
        graph.definitions.get(root_helper).map(String::as_str),
        Some("xtask/src/main.rs")
    );
    assert_eq!(
        graph.definitions.get(migration_helper).map(String::as_str),
        Some("xtask/src/openminis_ui_migration/tests/common.rs")
    );
    assert_ne!(
        graph.callers.get(root_helper),
        graph.callers.get(migration_helper),
        "same bare function names must retain independent module-qualified caller sets"
    );

    let path = root.join("docs/mainline-calls/foundation.workspace.json");
    let mut mainline: Value =
        serde_json::from_str(&fs::read_to_string(path).expect("read mainline"))
            .expect("parse mainline");
    mainline["shared_functions"][0]["symbol"] = Value::String(
        mainline["shared_functions"][0]["symbol"]
            .as_str()
            .expect("qualified symbol")
            .rsplit("::")
            .next()
            .expect("bare name")
            .to_owned(),
    );
    let err = verify_openminis_ui_call_graph_value(&root, &mainline)
        .expect_err("bare registry symbol must fail");
    assert!(err.contains("module-qualified"), "{err}");
}

#[test]
fn openminis_ui_migration_call_graph_includes_impl_trait_and_method_calls() {
    let root = test_repo_root("call-graph-methods");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;

impl Worker {
    fn helper(&self) {}
    fn associated() {}
    fn run(&self) {
        self.helper();
        Self::associated();
    }
}

trait Runner {
    fn execute(&self) {
        Worker::associated();
    }
}

impl Runner for Worker {
    fn execute(&self) {
        self.helper();
    }
}

fn invoke(worker: &Worker) {
    worker.run();
    worker.execute();
}

fn main() {}
"#,
    )
    .expect("method source");

    let graph = discover_rust_call_graph(&root).expect("discover method call graph");
    let helper = "crate::Worker::helper";
    let associated = "crate::Worker::associated";
    let run = "crate::Worker::run";
    let trait_default = "crate::Runner::execute";
    let trait_impl = "crate::Worker::impl_Runner::execute";

    for symbol in [helper, associated, run, trait_default, trait_impl] {
        assert_eq!(
            graph.definitions.get(symbol).map(String::as_str),
            Some("xtask/src/main.rs"),
            "missing method definition {symbol}"
        );
    }
    assert_eq!(
        graph.callers.get(helper),
        Some(&BTreeSet::from([run.to_owned(), trait_impl.to_owned()]))
    );
    assert_eq!(
        graph.callers.get(associated),
        Some(&BTreeSet::from([run.to_owned(), trait_default.to_owned()]))
    );
    assert_eq!(
        graph.callers.get(run),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
    assert_eq!(
        graph.callers.get(trait_impl),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_resolves_imported_receiver_types() {
    let root = test_repo_root("call-graph-imported-receiver");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod inner;
use crate::inner::Worker;

fn invoke(worker: Worker) {
    worker.run();
}

fn main() {}
"#,
    )
    .expect("main source");
    fs::write(
        root.join("xtask/src/inner.rs"),
        r#"
pub struct Worker;

impl Worker {
    pub fn run(&self) {}
}
"#,
    )
    .expect("inner source");

    let graph = discover_rust_call_graph(&root).expect("discover imported receiver call graph");
    assert_eq!(
        graph.callers.get("crate::inner::Worker::run"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_resolves_imported_impl_receiver_types() {
    let root = test_repo_root("call-graph-imported-impl-receiver");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod impls;
mod types;
use crate::types::Worker;

fn invoke(worker: Worker) {
    worker.run();
}

fn main() {}
"#,
    )
    .expect("main source");
    fs::write(root.join("xtask/src/types.rs"), "pub struct Worker;\n").expect("types source");
    fs::write(
        root.join("xtask/src/impls.rs"),
        r#"
use crate::types::Worker;

impl Worker {
    pub fn run(&self) {}
}
"#,
    )
    .expect("impl source");

    let graph = discover_rust_call_graph(&root).expect("discover imported impl receiver graph");
    assert_eq!(
        graph.callers.get("crate::types::Worker::run"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_rejects_ufcs_calls() {
    let root = test_repo_root("call-graph-ufcs-rejected");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
trait Runner { fn execute(&self); }
impl Runner for Worker { fn execute(&self) {} }

fn invoke(worker: &Worker) {
    <Worker as Runner>::execute(worker);
}

fn main() {}
"#,
    )
    .expect("ufcs source");

    let err = discover_rust_call_graph(&root).expect_err("UFCS must fail closed");
    assert!(err.contains("qualified UFCS call"), "{err}");
}

#[test]
fn openminis_ui_migration_call_graph_derives_test_identity_from_attributes() {
    let root = test_repo_root("call-graph-test-identity");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn target() {}

mod tests {
    pub fn production_caller() {
        super::target();
    }
}

#[test]
fn top_level_test() {
    target();
}

#[cfg(test)]
mod unit {
    pub fn cfg_test_caller() {
        super::target();
    }
}

fn main() {}
"#,
    )
    .expect("test identity source");

    let graph = discover_rust_call_graph(&root).expect("discover structured test identities");
    assert!(
        !graph
            .test_symbols
            .contains("crate::tests::production_caller")
    );
    assert!(graph.test_symbols.contains("crate::top_level_test"));
    assert!(graph.test_symbols.contains("crate::unit::cfg_test_caller"));
}

#[test]
fn openminis_ui_migration_call_graph_rejects_unresolved_local_methods() {
    let root = test_repo_root("call-graph-unresolved-local-method");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;

impl Worker {
    fn run(&self) {}
}

fn invoke(worker: &Worker) {
    worker.missing();
}

fn main() {}
"#,
    )
    .expect("unresolved local method source");

    let err = discover_rust_call_graph(&root)
        .expect_err("unresolved method on a repository-local receiver must fail");
    assert!(err.contains("cannot resolve local method call"), "{err}");
}

#[test]
fn openminis_ui_migration_call_graph_rejects_unbound_module_files() {
    let root = test_repo_root("call-graph-module-binding");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(root.join("xtask/src/main.rs"), "fn main() {}\n").expect("main source");
    fs::write(root.join("xtask/src/rogue.rs"), "fn hidden() {}\n").expect("rogue source");

    let err = discover_rust_call_graph(&root).expect_err("undeclared Rust module file must fail");
    assert!(err.contains("undeclared files"), "{err}");

    fs::remove_file(root.join("xtask/src/rogue.rs")).expect("remove rogue source");
    fs::write(
        root.join("xtask/src/main.rs"),
        "mod missing;\nfn main() {}\n",
    )
    .expect("missing module declaration");
    let err =
        discover_rust_call_graph(&root).expect_err("declared module without source file must fail");
    assert!(err.contains("declared modules without files"), "{err}");
}

#[test]
fn openminis_ui_migration_call_graph_rejects_cfg_disabled_callers() {
    let root = test_repo_root("call-graph-inactive-cfg");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        "#[cfg(any())]\nfn inactive() { active(); }\nfn active() {}\nfn main() {}\n",
    )
    .expect("cfg source");
    let err = discover_rust_call_graph(&root).expect_err("inactive cfg caller must fail");
    assert!(err.contains("disabled by cfg"), "{err}");

    fs::write(
        root.join("xtask/src/main.rs"),
        "#[cfg_attr(unix, allow(dead_code))]\nfn conditional_attr() {}\nfn main() {}\n",
    )
    .expect("cfg_attr source");
    let err = discover_rust_call_graph(&root).expect_err("unsupported cfg_attr must fail");
    assert!(err.contains("unsupported cfg_attr"), "{err}");

    fs::write(
        root.join("xtask/src/main.rs"),
        "#[cfg(unix)]\nfn active_on_current_target() {}\n#[cfg(test)]\nfn explicit_test_graph() {}\nfn main() {}\n",
    )
    .expect("active cfg source");
    let graph = discover_rust_call_graph(&root).expect("current target and test cfg must pass");
    assert!(
        graph
            .definitions
            .contains_key("crate::active_on_current_target")
    );
    assert!(graph.definitions.contains_key("crate::explicit_test_graph"));

    fs::write(
        root.join("xtask/src/main.rs"),
        "fn active() {\n  #[cfg(any())]\n  fn inactive_nested() { target(); }\n}\nfn target() {}\nfn main() {}\n",
    )
    .expect("nested inactive cfg source");
    let graph = discover_rust_call_graph(&root)
        .expect("cfg-disabled nested caller must not satisfy the outer caller");
    assert_eq!(graph.callers.get("crate::target"), Some(&BTreeSet::new()));

    fs::write(
        root.join("xtask/src/main.rs"),
        "fn active() {\n  fn nested() { target(); }\n}\nfn target() {}\nfn main() {}\n",
    )
    .expect("active nested source");
    let err = discover_rust_call_graph(&root)
        .expect_err("nested caller without an independent identity must fail explicitly");
    assert!(
        err.contains("independent module-qualified caller identity"),
        "{err}"
    );
}
