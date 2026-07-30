use super::*;
use syn::UseTree;

pub(super) fn flatten_use_tree(tree: &UseTree, prefix: Vec<String>, output: &mut Vec<RustImport>) {
    match tree {
        UseTree::Path(path) => {
            let mut next = prefix;
            next.push(path.ident.to_string());
            flatten_use_tree(&path.tree, next, output);
        }
        UseTree::Name(name) => {
            if name.ident == "self" && !prefix.is_empty() {
                output.push(RustImport {
                    target: prefix.clone(),
                    alias: prefix.last().cloned(),
                    glob: false,
                });
                return;
            }
            let mut target = prefix;
            target.push(name.ident.to_string());
            output.push(RustImport {
                target,
                alias: Some(name.ident.to_string()),
                glob: false,
            });
        }
        UseTree::Rename(rename) => {
            if rename.ident == "self" && !prefix.is_empty() {
                output.push(RustImport {
                    target: prefix,
                    alias: Some(rename.rename.to_string()),
                    glob: false,
                });
                return;
            }
            let mut target = prefix;
            target.push(rename.ident.to_string());
            output.push(RustImport {
                target,
                alias: Some(rename.rename.to_string()),
                glob: false,
            });
        }
        UseTree::Glob(_) => output.push(RustImport {
            target: prefix,
            alias: None,
            glob: true,
        }),
        UseTree::Group(group) => {
            for item in &group.items {
                flatten_use_tree(item, prefix.clone(), output);
            }
        }
    }
}

pub(super) fn resolve_call_path(
    module: &[String],
    path: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    definitions: &BTreeMap<String, String>,
) -> Result<BTreeSet<String>, String> {
    if path.len() == 1 {
        return visible_functions(module, &path[0], modules, definitions, &mut BTreeSet::new());
    }
    let scope = modules
        .get(&module.join("::"))
        .ok_or_else(|| format!("missing Rust module scope `{}`", module.join("::")))?;
    let mut candidates = BTreeSet::new();
    for import in &scope.imports {
        if import.glob {
            let mut target = normalize_use_target(module, &import.target);
            target.extend_from_slice(path);
            candidates.insert(target.join("::"));
        } else if import.alias.as_deref() == path.first().map(String::as_str) {
            let mut target = normalize_use_target(module, &import.target);
            target.extend_from_slice(&path[1..]);
            candidates.insert(target.join("::"));
        }
    }
    candidates.insert(normalize_use_target(module, path).join("::"));
    let mut resolved = BTreeSet::new();
    for candidate in candidates {
        if definitions.contains_key(&candidate) {
            resolved.insert(candidate);
            continue;
        }
        let mut segments = candidate
            .split("::")
            .map(ToOwned::to_owned)
            .collect::<Vec<_>>();
        let Some(name) = segments.pop() else {
            continue;
        };
        if modules.contains_key(&segments.join("::")) {
            resolved.extend(visible_functions(
                &segments,
                &name,
                modules,
                definitions,
                &mut BTreeSet::new(),
            )?);
        }
    }
    Ok(resolved)
}

pub(super) fn resolve_callable_path(
    module: &[String],
    call_scope: &[String],
    path: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    definitions: &BTreeMap<String, String>,
) -> Result<BTreeSet<String>, String> {
    let mut callable_candidate = call_scope.to_vec();
    callable_candidate.extend_from_slice(path);
    let callable_candidate = callable_candidate.join("::");
    if definitions.contains_key(&callable_candidate) {
        return Ok(BTreeSet::from([callable_candidate]));
    }
    let mut resolved = resolve_call_path(module, path, modules, definitions)?;
    let Some(scope) = modules.get(&call_scope.join("::")) else {
        return Ok(resolved);
    };
    let first = path.first().map(String::as_str);
    for import in &scope.imports {
        if import.glob {
            let mut target = normalize_use_target(module, &import.target);
            target.extend_from_slice(path);
            let candidate = target.join("::");
            if definitions.contains_key(&candidate) {
                resolved.insert(candidate);
            }
        } else if import.alias.as_deref() == first {
            let mut target = normalize_use_target(module, &import.target);
            target.extend_from_slice(&path[1..]);
            let candidate = target.join("::");
            if definitions.contains_key(&candidate) {
                resolved.insert(candidate);
            }
        }
    }
    Ok(resolved)
}

pub(super) fn lexical_import_can_reference_local(
    module: &[String],
    call_scope: &[String],
    path: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    definitions: &BTreeMap<String, String>,
) -> bool {
    let Some(scope) = modules.get(&call_scope.join("::")) else {
        return false;
    };
    let first = path.first().map(String::as_str);
    scope.imports.iter().any(|import| {
        let mut target = normalize_use_target(module, &import.target);
        if import.glob {
            target.extend_from_slice(path);
        } else if import.alias.as_deref() == first {
            target.extend_from_slice(&path[1..]);
        } else {
            return false;
        }
        let candidate = target.join("::");
        definitions.contains_key(&candidate)
            || definitions
                .keys()
                .any(|symbol| symbol.starts_with(&format!("{candidate}::")))
    })
}

pub(super) fn path_can_reference_local(
    module: &[String],
    path: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    definitions: &BTreeMap<String, String>,
) -> bool {
    if matches!(
        path.first().map(String::as_str),
        Some("crate" | "self" | "super")
    ) {
        return true;
    }
    let first = path.first().map(String::as_str).unwrap_or_default();
    let local_prefix = format!("{}::{first}::", module.join("::"));
    if definitions
        .keys()
        .any(|symbol| symbol.starts_with(&local_prefix))
    {
        return true;
    }
    modules
        .get(&module.join("::"))
        .into_iter()
        .flat_map(|scope| &scope.imports)
        .filter(|import| import.alias.as_deref() == Some(first))
        .map(|import| normalize_use_target(module, &import.target))
        .any(|target| {
            let prefix = format!("{}::", target.join("::"));
            definitions.keys().any(|symbol| symbol.starts_with(&prefix))
        })
}
