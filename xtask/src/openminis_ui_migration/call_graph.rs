use super::*;

const OPENMINIS_UI_MODULE_PATH: &str = "xtask/src/openminis_ui_migration/";
const OPENMINIS_UI_TEST_PATH: &str = "xtask/src/openminis_ui_migration/tests/";

#[derive(Clone, Debug)]
struct RustImport {
    target: Vec<String>,
    alias: Option<String>,
    glob: bool,
}

#[derive(Clone, Default)]
struct RustModuleScope {
    definitions: BTreeMap<String, String>,
    imports: Vec<RustImport>,
}

struct RustFunctionCalls {
    id: String,
    module: Vec<String>,
    call_scope: Vec<String>,
    paths: Vec<Vec<String>>,
    typed_method_paths: Vec<Vec<String>>,
    unresolved_method_names: Vec<String>,
    method_owner: Option<Vec<String>>,
    receiver_type: Option<Vec<String>>,
    self_method_names: Vec<String>,
    is_test: bool,
}

#[derive(Debug)]
pub(super) struct RustCallGraph {
    pub(super) definitions: BTreeMap<String, String>,
    pub(super) callers: BTreeMap<String, BTreeSet<String>>,
    pub(super) test_symbols: BTreeSet<String>,
}

pub(super) fn verify_openminis_ui_call_graph(root: &Path) -> Result<(), String> {
    let mainline_path = root.join("docs/mainline-calls/foundation.workspace.json");
    let mainline_raw = fs::read_to_string(&mainline_path)
        .map_err(|err| format!("read {}: {err}", mainline_path.display()))?;
    let mainline: Value = serde_json::from_str(&mainline_raw)
        .map_err(|err| format!("parse {}: {err}", mainline_path.display()))?;
    verify_openminis_ui_call_graph_value(root, &mainline)
}

pub(super) fn verify_openminis_ui_call_graph_value(
    root: &Path,
    mainline: &Value,
) -> Result<(), String> {
    let shared_functions = mainline
        .get("shared_functions")
        .and_then(Value::as_array)
        .ok_or_else(|| "foundation.workspace mainline missing shared_functions".to_owned())?;
    let graph = discover_rust_call_graph(root)?;
    let required_module_symbols = graph
        .definitions
        .iter()
        .filter(|(symbol, owner)| {
            owner.starts_with(OPENMINIS_UI_MODULE_PATH)
                && !owner.starts_with(OPENMINIS_UI_TEST_PATH)
                && graph
                    .callers
                    .get(*symbol)
                    .is_some_and(|callers| callers.len() > 1)
        })
        .map(|(symbol, _)| symbol.clone())
        .collect::<BTreeSet<_>>();
    let mut registered_module_symbols = BTreeSet::new();
    let mut registered_symbols = BTreeSet::new();
    for row in shared_functions {
        let row = row.as_object().ok_or_else(|| {
            "foundation.workspace shared_functions must contain objects".to_owned()
        })?;
        let symbol = required_string(row, "symbol", "foundation.workspace shared function")?;
        let owner = required_string(row, "owner", &format!("shared function {symbol}"))?;
        if !symbol.starts_with("crate::") {
            return Err(format!(
                "OpenMinis UI shared function `{symbol}` must use a module-qualified crate identity"
            ));
        }
        if !registered_symbols.insert(symbol.to_owned()) {
            return Err(format!(
                "OpenMinis UI call graph has duplicate shared function `{symbol}`"
            ));
        }
        let actual_owner = graph.definitions.get(symbol).ok_or_else(|| {
            format!("OpenMinis UI shared function `{symbol}` has no Rust definition")
        })?;
        if actual_owner != owner {
            return Err(format!(
                "OpenMinis UI shared function `{symbol}` owner drift: declared=`{owner}`, actual=`{actual_owner}`"
            ));
        }
        if owner.starts_with(OPENMINIS_UI_MODULE_PATH) && !owner.starts_with(OPENMINIS_UI_TEST_PATH)
        {
            registered_module_symbols.insert(symbol.to_owned());
        }
        let declared_callers = string_array(
            row.get("allowed_callers"),
            &format!("shared function {symbol} allowed_callers"),
        )?;
        let actual_callers = graph.callers.get(symbol).cloned().unwrap_or_default();
        if declared_callers != actual_callers {
            return Err(format!(
                "OpenMinis UI shared function `{symbol}` caller drift: declared={declared_callers:?}, actual={actual_callers:?}"
            ));
        }
        let declared_tests = string_array(
            row.get("related_tests"),
            &format!("shared function {symbol} related_tests"),
        )?;
        let actual_tests = actual_callers
            .iter()
            .filter(|caller| graph.test_symbols.contains(*caller))
            .cloned()
            .collect::<BTreeSet<_>>();
        if declared_tests != actual_tests {
            return Err(format!(
                "OpenMinis UI shared function `{symbol}` test mapping drift: declared={declared_tests:?}, actual={actual_tests:?}"
            ));
        }
    }
    if registered_module_symbols != required_module_symbols {
        return Err(format!(
            "OpenMinis UI shared-function registry drift: required={required_module_symbols:?}, registered={registered_module_symbols:?}"
        ));
    }
    verify_openminis_ui_call_edges(mainline, &graph)
}

fn verify_openminis_ui_call_edges(mainline: &Value, graph: &RustCallGraph) -> Result<(), String> {
    let required_edges = required_openminis_ui_call_edges(graph);

    let rows = mainline
        .get("call_table")
        .and_then(Value::as_array)
        .ok_or_else(|| "foundation.workspace mainline missing call_table".to_owned())?;
    let mut registered_edges = BTreeSet::new();
    for row in rows {
        let row = row
            .as_object()
            .ok_or_else(|| "foundation.workspace call_table must contain objects".to_owned())?;
        if row.get("graph_id").and_then(Value::as_str) != Some("openminis_ui_migration") {
            continue;
        }
        let step = required_string(row, "step", "OpenMinis UI call edge")?;
        let caller = required_string(row, "caller", &format!("OpenMinis UI call edge {step}"))?;
        let callee = required_string(row, "callee", &format!("OpenMinis UI call edge {step}"))?;
        let symbol_path = required_string(
            row,
            "symbol_path",
            &format!("OpenMinis UI call edge {step}"),
        )?;
        let file_path =
            required_string(row, "file_path", &format!("OpenMinis UI call edge {step}"))?;
        if !caller.starts_with("crate::") || !callee.starts_with("crate::") {
            return Err(format!(
                "OpenMinis UI call edge `{step}` must use module-qualified caller and callee identities"
            ));
        }
        if symbol_path != callee {
            return Err(format!(
                "OpenMinis UI call edge `{step}` symbol_path must equal callee: `{symbol_path}` != `{callee}`"
            ));
        }
        if graph.definitions.get(callee).map(String::as_str) != Some(file_path) {
            return Err(format!(
                "OpenMinis UI call edge `{step}` callee `{callee}` does not resolve in `{file_path}`"
            ));
        }
        let edge = (caller.to_owned(), callee.to_owned());
        if !registered_edges.insert(edge.clone()) {
            return Err(format!(
                "OpenMinis UI call edge `{step}` duplicates {edge:?}"
            ));
        }
    }
    if registered_edges != required_edges {
        let missing = required_edges
            .difference(&registered_edges)
            .cloned()
            .collect::<BTreeSet<_>>();
        let extra = registered_edges
            .difference(&required_edges)
            .cloned()
            .collect::<BTreeSet<_>>();
        return Err(format!(
            "OpenMinis UI direct call-edge registry drift: missing={missing:?}, extra={extra:?}"
        ));
    }
    Ok(())
}

fn is_migration_production_symbol(graph: &RustCallGraph, symbol: &str) -> bool {
    graph.definitions.get(symbol).is_some_and(|owner| {
        owner.starts_with(OPENMINIS_UI_MODULE_PATH) && !owner.starts_with(OPENMINIS_UI_TEST_PATH)
    })
}

pub(super) fn required_openminis_ui_call_edges(
    graph: &RustCallGraph,
) -> BTreeSet<(String, String)> {
    let mut edges = BTreeSet::new();
    for (callee, callers) in &graph.callers {
        let migration_callee = is_migration_production_symbol(graph, callee);
        for caller in callers {
            if graph.test_symbols.contains(caller) {
                continue;
            }
            if migration_callee || is_migration_production_symbol(graph, caller) {
                edges.insert((caller.clone(), callee.clone()));
            }
        }
    }
    edges
}

mod cfg;
mod discovery;
pub(super) use discovery::discover_rust_call_graph;
