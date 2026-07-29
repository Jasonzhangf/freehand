#[test]
fn openminis_ui_migration_call_graph_indexes_imports_before_callables() {
    let root = test_repo_root("call-graph-late-import");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod inner;

fn invoke(worker: Worker) {
    worker.run();
}

use crate::inner::Worker;

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

    let graph = discover_rust_call_graph(&root).expect("discover late-import receiver graph");
    assert_eq!(
        graph.callers.get("crate::inner::Worker::run"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_resolves_or_rejects_non_identifier_receivers() {
    let root = test_repo_root("call-graph-unresolved-receiver-forms");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    for (case, caller) in [
        (
            "call-result",
            "fn invoke() { make_worker().run(); }\nfn make_worker() -> Worker { Worker }",
        ),
        (
            "inferred-local",
            "fn invoke() { let worker = make_worker(); worker.run(); }\nfn make_worker() -> Worker { Worker }",
        ),
    ] {
        fs::write(
            root.join("xtask/src/main.rs"),
            format!(
                "struct Worker;\nimpl Worker {{ fn run(&self) {{}} }}\n{caller}\nfn main() {{}}\n"
            ),
        )
        .expect("receiver fixture");
        let graph = discover_rust_call_graph(&root).expect("return type should resolve receiver");
        assert_eq!(
            graph.callers.get("crate::Worker::run"),
            Some(&BTreeSet::from(["crate::invoke".to_owned()])),
            "{case}"
        );
    }

    fs::write(
        root.join("xtask/src/main.rs"),
        "struct Worker;\nimpl Worker { fn run(&self) {} }\nstruct Host { worker: Worker }\nimpl Host { fn invoke(&self) { self.worker.run(); } }\nfn main() {}\n",
    )
    .expect("field receiver fixture");
    let graph = discover_rust_call_graph(&root).expect("typed self field should resolve receiver");
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::from(["crate::Host::invoke".to_owned()]))
    );

    fs::write(
        root.join("xtask/src/main.rs"),
        "struct Worker;\nimpl Worker { fn run(&self) {} }\nfn invoke(condition: bool) { let worker = if condition { Worker } else { Worker }; worker.run(); }\nfn main() {}\n",
    )
    .expect("unresolved identifier receiver fixture");
    let graph =
        discover_rust_call_graph(&root).expect("conditional local receiver type must resolve");
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );

    fs::write(
        root.join("xtask/src/main.rs"),
        "struct Worker;\ntrait Runner { fn execute(&self); }\nimpl Runner for Worker { fn execute(&self) {} }\nfn invoke(worker: &Worker) { Runner::execute(worker); }\nfn main() {}\n",
    )
    .expect("trait-qualified call fixture");
    let err = discover_rust_call_graph(&root).expect_err("trait-qualified call must fail closed");
    assert!(
        err.contains("trait-qualified call `Runner::execute`"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_call_graph_resolves_unwrap_identifier_receiver() {
    let root = test_repo_root("call-graph-untyped-identifier-receiver");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) {} }
fn factory() -> Result<Worker, ()> { Ok(Worker) }
fn invoke() {
    let worker = factory().unwrap();
    worker.run();
}
fn main() {}
"#,
    )
    .expect("untyped receiver fixture");

    let graph =
        discover_rust_call_graph(&root).expect("unwrap success type must resolve local receiver");
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_rejects_unexpanded_local_and_include_macros() {
    let root = test_repo_root("call-graph-unexpanded-macros");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    for (name, source, expected) in [
        (
            "local-definition",
            "macro_rules! invoke { () => { local() } }\nfn local() {}\nfn main() { invoke!(); }\n",
            "item macro `macro_rules!`",
        ),
        (
            "include-expression",
            "fn local() {}\nfn main() { include!(\"calls.rs\"); }\n",
            "macro `include!`",
        ),
    ] {
        fs::write(root.join("xtask/src/main.rs"), source).expect("macro fixture");
        let err = discover_rust_call_graph(&root)
            .expect_err("unexpanded macro source must not forge a complete call graph");
        assert!(err.contains(expected), "{name}: {err}");
        assert!(err.contains("may hide local call edges"), "{name}: {err}");
    }
}

#[test]
fn openminis_ui_migration_call_graph_inspects_or_rejects_ordinary_macro_calls() {
    let root = test_repo_root("call-graph-ordinary-macros");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        "fn target() -> bool { true }\nfn invoke() { assert!(target()); }\nfn main() {}\n",
    )
    .expect("parseable macro fixture");
    let graph = discover_rust_call_graph(&root).expect("macro expression calls must be inspected");
    assert_eq!(
        graph.callers.get("crate::target"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );

    fs::write(
        root.join("xtask/src/main.rs"),
        "fn target() -> usize { 1 }\nfn invoke() { opaque!(target() @ value); }\nfn main() {}\n",
    )
    .expect("opaque macro fixture");
    let err = discover_rust_call_graph(&root)
        .expect_err("opaque macro body with call-like tokens must fail closed");
    assert!(err.contains("opaque macro"), "{err}");
    assert!(err.contains("may hide local call edges"), "{err}");
}

#[test]
fn openminis_ui_migration_call_graph_rejects_chained_potentially_local_methods() {
    let root = test_repo_root("call-graph-chained-local-method");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker {
    fn prepare(&self) -> Worker { Worker }
    fn run(&self) {}
}
fn make_worker() -> Worker { Worker }
fn invoke() { make_worker().prepare().run(); }
fn main() {}
"#,
    )
    .expect("chained method fixture");
    let err = discover_rust_call_graph(&root)
        .expect_err("chained potentially local method must resolve or fail closed");
    assert!(
        err.contains("cannot resolve potentially local method call `run`"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_call_graph_inherits_test_identity_for_methods() {
    let root = test_repo_root("call-graph-test-method-identity");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
fn shared() {}

#[cfg(test)]
mod checks {
    use super::shared;

    struct Worker;
    impl Worker {
        fn run(&self) { shared(); }
    }

    trait Runner {
        fn execute(&self) { shared(); }
    }
}

fn main() {}
"#,
    )
    .expect("test method fixture");

    let graph = discover_rust_call_graph(&root).expect("discover test method graph");
    for symbol in [
        "crate::checks::Worker::run",
        "crate::checks::Runner::execute",
    ] {
        assert!(
            graph.test_symbols.contains(symbol),
            "{symbol} must remain test-only"
        );
    }
    assert_eq!(
        graph.callers.get("crate::shared"),
        Some(&BTreeSet::from([
            "crate::checks::Runner::execute".to_owned(),
            "crate::checks::Worker::run".to_owned(),
        ]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_tracks_closure_and_lexical_receiver_types() {
    let root = test_repo_root("call-graph-lexical-receivers");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) {} }
fn invoke() {
    let call = |worker: &Worker| worker.run();
    let worker = Worker;
    { let worker: String = String::new(); let _ = worker; }
    worker.run();
    call(&worker);
}
fn main() {}
"#,
    )
    .expect("lexical receiver fixture");

    let graph = discover_rust_call_graph(&root).expect("discover lexical receiver graph");
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_rejects_unknown_shadow_receivers() {
    let root = test_repo_root("call-graph-unknown-shadow-receiver");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) {} }
fn external_value() -> String { String::new() }
fn invoke() {
    let worker = Worker;
    {
        let worker = external_value().into_boxed_str();
        worker.run();
    }
    worker.run();
}
fn main() {}
"#,
    )
    .expect("unknown shadow receiver fixture");

    let err = discover_rust_call_graph(&root)
        .expect_err("unknown shadow receiver with a local method name must fail closed");
    assert!(
        err.contains("cannot resolve potentially local method call `run`"),
        "{err}"
    );
}

#[test]
fn openminis_ui_migration_call_graph_treats_external_iterator_items_as_external() {
    let root = test_repo_root("call-graph-external-iterator-item");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Local;
impl Local { fn extension(&self) {} }
fn invoke() {
    let paths = Vec::new();
    for path in paths {
        let _ = path.extension();
    }
}
fn main() {}
"#,
    )
    .expect("external iterator item fixture");

    discover_rust_call_graph(&root)
        .expect("external iterator item methods must not become local call edges");
}

#[test]
fn openminis_ui_migration_call_graph_tracks_external_method_chain_bindings() {
    let root = test_repo_root("call-graph-external-method-chain-binding");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Local;
impl Local { fn push(&self) {} }
fn invoke(input: &str) -> Result<(), String> {
    let relative = input.strip_prefix("x").ok_or_else(|| "missing".to_owned())?;
    let mut components = relative.split('/').collect::<Vec<_>>();
    components.push("tail");
    Ok(())
}
fn main() {}
"#,
    )
    .expect("external method-chain binding fixture");

    discover_rust_call_graph(&root)
        .expect("bindings rooted in typed external receivers must remain external");
}

#[test]
fn openminis_ui_migration_call_graph_treats_builtin_signature_receivers_as_external() {
    let root = test_repo_root("call-graph-builtin-signature-receiver");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Local;
impl Local { fn push(&self) {} }
fn invoke(parent: &[String]) {
    let mut values = parent.to_vec();
    values.push("value".to_owned());
}
fn main() {}
"#,
    )
    .expect("builtin signature receiver fixture");

    discover_rust_call_graph(&root)
        .expect("builtin signature receivers and their method chains must remain external");
}

#[test]
fn openminis_ui_migration_call_graph_tracks_smart_pointer_autoderef() {
    let root = test_repo_root("call-graph-smart-pointer");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
struct Worker;
impl Worker { fn run(&self) {} }
fn invoke(worker: Box<Worker>) { worker.run(); }
fn main() {}
"#,
    )
    .expect("smart pointer fixture");

    let graph = discover_rust_call_graph(&root).expect("discover autoderef receiver graph");
    assert_eq!(
        graph.callers.get("crate::Worker::run"),
        Some(&BTreeSet::from(["crate::invoke".to_owned()]))
    );
}

#[test]
fn openminis_ui_migration_call_graph_resolves_grouped_self_imports() {
    let root = test_repo_root("call-graph-grouped-self-import");
    fs::create_dir_all(root.join("xtask/src")).expect("xtask src");
    fs::write(
        root.join("xtask/src/main.rs"),
        r#"
mod parent { pub fn target() {} }
mod first {
    use crate::parent::{self};
    pub fn invoke() { parent::target(); }
}
mod second {
    use crate::parent::{self as imported};
    pub fn invoke() { imported::target(); }
}
fn main() {}
"#,
    )
    .expect("grouped self import fixture");

    let graph = discover_rust_call_graph(&root).expect("discover grouped self imports");
    assert_eq!(
        graph.callers.get("crate::parent::target"),
        Some(&BTreeSet::from([
            "crate::first::invoke".to_owned(),
            "crate::second::invoke".to_owned(),
        ]))
    );
}
use super::*;
