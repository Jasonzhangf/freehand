use super::cfg::{active_in_cfg_projection, active_rust_cfg};
use super::*;
use syn::{Item, ItemFn, ItemMod};

mod attributes;
mod index;
mod macros;
mod methods;
mod scope;
mod state;
use attributes::has_test_attribute;
use index::{
    collect_module_test_relations, index_function_return_types, index_module_items,
    index_struct_fields, item_attrs, resolve_reachable_modules, resolve_test_modules,
};
use macros::reject_unexpanded_call_sources;
use methods::*;
use scope::{
    flatten_use_tree, lexical_import_can_reference_local, path_can_reference_local,
    resolve_callable_path,
};
use state::DiscoveryState;

pub(in crate::openminis_ui_migration) fn discover_rust_call_graph(
    root: &Path,
) -> Result<RustCallGraph, String> {
    let active_cfg = active_rust_cfg()?;
    let (mut production, file_modules, mut declared_modules) =
        discover_rust_call_graph_projection(root, &active_cfg)?;
    let mut test_cfg = active_cfg;
    test_cfg.insert("test".to_owned());
    let (test, test_files, test_declared_modules) =
        discover_rust_call_graph_projection(root, &test_cfg)?;
    if file_modules != test_files {
        return Err("Rust production/test projection file graph drift".to_owned());
    }
    declared_modules.extend(test_declared_modules);
    let undeclared = file_modules
        .iter()
        .filter(|module| module.as_str() != "crate")
        .filter(|module| !declared_modules.contains(*module))
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing_files = declared_modules
        .difference(&file_modules)
        .cloned()
        .collect::<BTreeSet<_>>();
    if !undeclared.is_empty() || !missing_files.is_empty() {
        return Err(format!(
            "Rust module/file graph drift: undeclared files={undeclared:?}, declared modules without files={missing_files:?}"
        ));
    }

    let production_symbols = production
        .definitions
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    for (symbol, owner) in test.definitions {
        production.definitions.entry(symbol).or_insert(owner);
    }
    for (callee, callers) in test.callers {
        production.callers.entry(callee).or_default().extend(
            callers
                .into_iter()
                .filter(|caller| !production_symbols.contains(caller)),
        );
    }
    production.test_symbols.extend(
        test.test_symbols
            .into_iter()
            .filter(|symbol| !production_symbols.contains(symbol)),
    );
    Ok(production)
}

fn discover_rust_call_graph_projection(
    root: &Path,
    active_cfg: &BTreeSet<String>,
) -> Result<(RustCallGraph, BTreeSet<String>, BTreeSet<String>), String> {
    let mut files = Vec::new();
    collect_regular_files(&root.join("xtask/src"), &mut files)?;
    let mut modules = BTreeMap::<String, RustModuleScope>::new();
    let mut functions = Vec::new();
    let mut definitions = BTreeMap::<String, String>::new();
    let mut method_dispatch = BTreeMap::new();
    let mut local_types = BTreeSet::new();
    let mut local_traits = BTreeSet::new();
    let mut function_return_types = BTreeMap::new();
    let mut function_try_return_types = BTreeMap::new();
    let mut struct_fields = BTreeMap::new();
    let mut trait_impls = Vec::new();
    let mut deref_targets = BTreeMap::new();
    let mut trait_default_methods = BTreeMap::new();
    let mut file_modules = BTreeSet::new();
    let mut declared_external_modules = BTreeSet::new();
    let mut parsed_files = Vec::new();
    for file in files {
        if file.extension().and_then(|value| value.to_str()) != Some("rs") {
            continue;
        }
        let owner = file
            .strip_prefix(root)
            .map_err(|err| format!("Rust call-graph source escaped repository: {err}"))?
            .to_string_lossy()
            .replace('\\', "/");
        let module = rust_file_module(&owner)?;
        file_modules.insert(module.join("::"));
        let source = fs::read_to_string(&file)
            .map_err(|err| format!("read Rust call-graph source {}: {err}", file.display()))?;
        let syntax = syn::parse_file(&source)
            .map_err(|err| format!("parse Rust call-graph source {}: {err}", file.display()))?;
        reject_unexpanded_call_sources(&syntax, &owner)?;
        parsed_files.push((owner, module, syntax));
    }
    let mut test_relations = Vec::new();
    for (_, module, syntax) in &parsed_files {
        collect_module_test_relations(&syntax.items, module, active_cfg, &mut test_relations)?;
    }
    let reachable_modules = resolve_reachable_modules(&test_relations);
    let test_modules = resolve_test_modules(&test_relations);
    let active_files = parsed_files
        .iter()
        .filter(|(_, module, _)| reachable_modules.contains(&module.join("::")))
        .collect::<Vec<_>>();
    {
        let mut state = DiscoveryState {
            modules: &mut modules,
            definitions: &mut definitions,
            functions: &mut functions,
            method_dispatch: &mut method_dispatch,
            local_types: &mut local_types,
            local_traits: &mut local_traits,
            function_return_types: &mut function_return_types,
            function_try_return_types: &mut function_try_return_types,
            struct_fields: &mut struct_fields,
            trait_impls: &mut trait_impls,
            deref_targets: &mut deref_targets,
            trait_default_methods: &mut trait_default_methods,
            declared_external_modules: &mut declared_external_modules,
            test_modules: &test_modules,
            active_cfg,
        };
        for (_, module, syntax) in &active_files {
            index_module_items(&syntax.items, module, &mut state)?;
        }
        for (_, module, syntax) in &active_files {
            index_function_return_types(&syntax.items, module, &mut state)?;
        }
        for (_, module, syntax) in &active_files {
            index_struct_fields(&syntax.items, module, &mut state)?;
        }
        for (owner, module, syntax) in &active_files {
            let inherited_test = state.test_modules.contains(&module.join("::"));
            collect_module_items(&syntax.items, module, owner, inherited_test, &mut state)?;
        }
    }
    inherit_trait_default_dispatch(&trait_impls, &trait_default_methods, &mut method_dispatch);
    let all_names = definitions
        .keys()
        .filter_map(|id| id.rsplit("::").next().map(ToOwned::to_owned))
        .collect::<BTreeSet<_>>();
    let external_trait_receivers = trait_impls
        .iter()
        .filter(|(_, trait_path)| !local_types.contains(&trait_path.join("::")))
        .map(|(receiver, _)| receiver.join("::"))
        .collect::<BTreeSet<_>>();
    let mut callers = definitions
        .keys()
        .map(|symbol| (symbol.clone(), BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    let local_method_names = method_dispatch
        .keys()
        .map(|(_, method)| method.clone())
        .collect::<BTreeSet<_>>();
    for function in &functions {
        for path in &function.paths {
            let Some(name) = path.last() else { continue };
            let resolved = resolve_callable_path(
                &function.module,
                &function.call_scope,
                path,
                &modules,
                &definitions,
            )?;
            if resolved.is_empty() {
                if !all_names.contains(name) {
                    continue;
                }
                if path_can_reference_local(&function.module, path, &modules, &definitions)
                    || lexical_import_can_reference_local(
                        &function.module,
                        &function.call_scope,
                        path,
                        &modules,
                        &definitions,
                    )
                {
                    return Err(format!(
                        "Rust call graph cannot resolve `{}` from module-qualified caller `{}`",
                        path.join("::"),
                        function.id
                    ));
                }
                continue;
            }
            if resolved.len() != 1 {
                return Err(format!(
                    "Rust call graph has ambiguous call `{}` from `{}`: {resolved:?}",
                    path.join("::"),
                    function.id
                ));
            }
            let callee = resolved.first().expect("one resolved callee");
            callers
                .get_mut(callee)
                .expect("defined symbol initialized")
                .insert(function.id.clone());
        }
        for path in &function.typed_method_paths {
            let (receiver, method) = path.split_at(path.len() - 1);
            let resolved = resolve_method_dispatch(
                &receiver.join("::"),
                &method[0],
                &method_dispatch,
                &deref_targets,
            );
            let receiver_id = receiver.join("::");
            let must_resolve = local_types.contains(&receiver_id)
                && !external_trait_receivers.contains(&receiver_id);
            record_method_edge(
                function,
                &path.join("::"),
                resolved,
                must_resolve,
                &mut callers,
            )?;
        }
        for name in &function.unresolved_method_names {
            if local_method_names.contains(name) {
                return Err(format!(
                    "Rust call graph cannot resolve potentially local method call `{name}` from `{}`",
                    function.id
                ));
            }
        }
        for name in &function.self_method_names {
            let mut resolved = BTreeSet::new();
            if let Some(receiver) = function.receiver_type.as_ref() {
                resolved.extend(resolve_method_dispatch(
                    &receiver.join("::"),
                    name,
                    &method_dispatch,
                    &deref_targets,
                ));
            }
            if let Some(owner) = function.method_owner.as_ref() {
                let candidate = format!("{}::{name}", owner.join("::"));
                if definitions.contains_key(&candidate) {
                    resolved.insert(candidate);
                }
            }
            let must_resolve = function.receiver_type.as_ref().is_some_and(|receiver| {
                let receiver = receiver.join("::");
                local_types.contains(&receiver) && !external_trait_receivers.contains(&receiver)
            });
            record_method_edge(function, name, resolved, must_resolve, &mut callers)?;
        }
    }
    let test_symbols = functions
        .iter()
        .filter(|function| function.is_test)
        .map(|function| function.id.clone())
        .collect();
    Ok((
        RustCallGraph {
            definitions,
            callers,
            test_symbols,
        },
        file_modules,
        declared_external_modules,
    ))
}

fn rust_file_module(owner: &str) -> Result<Vec<String>, String> {
    let relative = owner
        .strip_prefix("xtask/src/")
        .ok_or_else(|| format!("Rust call-graph owner is outside xtask/src: `{owner}`"))?;
    let mut components = relative
        .split('/')
        .map(ToOwned::to_owned)
        .collect::<Vec<_>>();
    let file = components
        .pop()
        .ok_or_else(|| format!("Rust call-graph owner has no file: `{owner}`"))?;
    if file != "main.rs" && file != "lib.rs" && file != "mod.rs" {
        components.push(
            file.strip_suffix(".rs")
                .ok_or_else(|| format!("Rust call-graph owner is not Rust: `{owner}`"))?
                .to_owned(),
        );
    }
    let mut module = vec!["crate".to_owned()];
    module.extend(components);
    Ok(module)
}

fn collect_module_items(
    items: &[Item],
    module: &[String],
    owner: &str,
    inherited_test: bool,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    let module_id = module.join("::");
    state.modules.entry(module_id.clone()).or_default();
    for item in items {
        let Some(attrs) = item_attrs(item) else {
            continue;
        };
        if !active_in_cfg_projection(attrs, &format!("item in `{module_id}`"), state.active_cfg)? {
            continue;
        }
        match item {
            Item::Fn(item_fn) => {
                collect_function(item_fn, module, owner, inherited_test, state)?;
            }
            Item::Mod(item_mod) => {
                collect_module(item_mod, module, owner, inherited_test, state)?;
            }
            Item::Impl(item_impl) => {
                collect_top_level_impl(item_impl, module, owner, inherited_test, state)?
            }
            Item::Trait(item_trait) => {
                collect_top_level_trait(item_trait, module, owner, inherited_test, state)?
            }
            Item::Const(item_const) => collect_initializer(
                &item_const.ident,
                &item_const.attrs,
                &item_const.expr,
                module,
                owner,
                inherited_test,
                state,
            )?,
            Item::Static(item_static) => collect_initializer(
                &item_static.ident,
                &item_static.attrs,
                &item_static.expr,
                module,
                owner,
                inherited_test,
                state,
            )?,
            Item::Enum(item_enum) => {
                collect_enum_discriminants(item_enum, module, owner, inherited_test, state)?
            }
            _ => {}
        }
    }
    Ok(())
}

fn collect_function(
    item: &ItemFn,
    module: &[String],
    owner: &str,
    inherited_test: bool,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    let name = item.sig.ident.to_string();
    let id = format!("{}::{name}", module.join("::"));
    state
        .modules
        .get_mut(&module.join("::"))
        .expect("module initialized")
        .definitions
        .insert(name, id.clone());
    let is_test = inherited_test || has_test_attribute(&item.attrs, state.active_cfg);
    collect_local_items(&item.block.stmts, module, &id, owner, is_test, state)?;
    collect_callable(
        CallableSource {
            id,
            module,
            owner,
            attrs: &item.attrs,
            sig: &item.sig,
            block: &item.block,
            method_owner: None,
            receiver_type: None,
            is_test,
        },
        state,
    )
}

fn collect_module(
    item: &ItemMod,
    parent: &[String],
    owner: &str,
    inherited_test: bool,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    let mut module = parent.to_vec();
    module.push(item.ident.to_string());
    let Some((_, items)) = &item.content else {
        state.declared_external_modules.insert(module.join("::"));
        return Ok(());
    };
    collect_module_items(
        items,
        &module,
        owner,
        inherited_test || has_test_attribute(&item.attrs, state.active_cfg),
        state,
    )
}

pub(super) fn visible_functions(
    module: &[String],
    name: &str,
    modules: &BTreeMap<String, RustModuleScope>,
    definitions: &BTreeMap<String, String>,
    visited: &mut BTreeSet<(String, String)>,
) -> Result<BTreeSet<String>, String> {
    let module_id = module.join("::");
    if !visited.insert((module_id.clone(), name.to_owned())) {
        return Ok(BTreeSet::new());
    }
    let scope = modules
        .get(&module_id)
        .ok_or_else(|| format!("missing Rust module scope `{module_id}`"))?;
    let mut output = BTreeSet::new();
    if let Some(id) = scope.definitions.get(name) {
        output.insert(id.clone());
    }
    for import in &scope.imports {
        if import.glob {
            let target = normalize_use_target(module, &import.target);
            if modules.contains_key(&target.join("::")) {
                output.extend(visible_functions(
                    &target,
                    name,
                    modules,
                    definitions,
                    visited,
                )?);
            }
        } else if import.alias.as_deref() == Some(name) {
            let target = normalize_use_target(module, &import.target).join("::");
            if definitions.contains_key(&target) {
                output.insert(target);
            }
        }
    }
    Ok(output)
}

pub(super) fn normalize_use_target(module: &[String], raw: &[String]) -> Vec<String> {
    let mut output;
    let mut index = 0;
    match raw.first().map(String::as_str) {
        Some("crate") => {
            output = vec!["crate".to_owned()];
            index = 1;
        }
        Some("self") => {
            output = module.to_vec();
            index = 1;
        }
        Some("super") => {
            output = module.to_vec();
            while raw.get(index).map(String::as_str) == Some("super") {
                if output.len() > 1 {
                    output.pop();
                }
                index += 1;
            }
        }
        _ => output = module.to_vec(),
    }
    output.extend_from_slice(&raw[index..]);
    output
}
