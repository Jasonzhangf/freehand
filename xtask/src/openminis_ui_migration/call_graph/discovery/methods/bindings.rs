use super::super::*;
use crate::openminis_ui_migration::call_graph::cfg::cfg_enabled;
use syn::visit::{self, Visit};
use syn::{Expr, Pat};

#[derive(Clone)]
pub(super) enum PatternBindingType {
    Local(Vec<String>),
    External,
    Unknown,
}

pub(super) fn collect_block_scope_bindings(
    block: &syn::Block,
    lexical_imports: &mut Vec<RustImport>,
    bound_values: &mut BTreeSet<String>,
    active_cfg: &BTreeSet<String>,
    caller_id: &str,
) -> Result<(), String> {
    for item in block.stmts.iter().filter_map(|statement| match statement {
        syn::Stmt::Item(item) => Some(item),
        _ => None,
    }) {
        let Some(attrs) = item_attrs(item) else {
            continue;
        };
        if !cfg_enabled(
            attrs,
            active_cfg,
            &format!("block item in module-qualified caller `{caller_id}`"),
        )? {
            continue;
        }
        match item {
            Item::Use(item_use) => {
                flatten_use_tree(&item_use.tree, Vec::new(), lexical_imports);
            }
            Item::Const(item_const) => {
                bound_values.insert(item_const.ident.to_string());
            }
            Item::Static(item_static) => {
                bound_values.insert(item_static.ident.to_string());
            }
            _ => {}
        }
    }
    Ok(())
}

pub(super) fn apply_pattern_binding_state(
    name: String,
    binding_type: Option<&PatternBindingType>,
    definitely_external: bool,
    local_types: &mut BTreeMap<String, Vec<String>>,
    known_external_locals: &mut BTreeSet<String>,
    iterable_item_types: &mut BTreeMap<String, Vec<String>>,
    bound_values: &mut BTreeSet<String>,
) {
    local_types.remove(&name);
    iterable_item_types.remove(&name);
    bound_values.insert(name.clone());
    match binding_type {
        Some(PatternBindingType::Local(owner)) => {
            known_external_locals.remove(&name);
            local_types.insert(name, owner.clone());
        }
        Some(PatternBindingType::External) => {
            known_external_locals.insert(name);
        }
        Some(PatternBindingType::Unknown) | None if definitely_external => {
            known_external_locals.insert(name);
        }
        Some(PatternBindingType::Unknown) | None => {
            known_external_locals.remove(&name);
        }
    }
}

pub(super) fn resolve_lexical_import(
    module: &[String],
    lexical_imports: &[RustImport],
    path: &[String],
    modules: &BTreeMap<String, RustModuleScope>,
    definitions: &BTreeMap<String, String>,
) -> Result<Option<Vec<String>>, String> {
    let Some(first) = path.first() else {
        return Ok(None);
    };
    if let Some(import) = lexical_imports
        .iter()
        .rev()
        .find(|import| !import.glob && import.alias.as_ref() == Some(first))
    {
        let mut target = normalize_use_target(module, &import.target);
        target.extend_from_slice(&path[1..]);
        return Ok(Some(target));
    }
    let mut candidates = BTreeSet::new();
    for import in lexical_imports.iter().filter(|import| import.glob) {
        let mut target = normalize_use_target(module, &import.target);
        if path.len() == 1 {
            candidates.extend(visible_functions(
                &target,
                first,
                modules,
                definitions,
                &mut BTreeSet::new(),
            )?);
        } else {
            target.extend_from_slice(path);
            if definitions.contains_key(&target.join("::")) {
                candidates.insert(target.join("::"));
            }
        }
    }
    match candidates.len() {
        0 => Ok(None),
        1 => Ok(candidates
            .into_iter()
            .next()
            .map(|symbol| symbol.split("::").map(ToOwned::to_owned).collect())),
        _ => Err(format!(
            "ambiguous lexical glob call `{}`: {candidates:?}",
            path.join("::")
        )),
    }
}

pub(super) fn pattern_identifiers(pattern: &Pat) -> Vec<String> {
    struct IdentifierVisitor {
        names: Vec<String>,
    }

    impl<'ast> Visit<'ast> for IdentifierVisitor {
        fn visit_pat_ident(&mut self, pattern: &'ast syn::PatIdent) {
            self.names.push(pattern.ident.to_string());
            visit::visit_pat_ident(self, pattern);
        }
    }

    let mut visitor = IdentifierVisitor { names: Vec::new() };
    visitor.visit_pat(pattern);
    visitor.names
}

pub(super) fn local_identifier(pattern: &Pat) -> Option<String> {
    match pattern {
        Pat::Ident(name) => Some(name.ident.to_string()),
        Pat::Type(typed) => {
            let Pat::Ident(name) = typed.pat.as_ref() else {
                return None;
            };
            Some(name.ident.to_string())
        }
        _ => None,
    }
}

pub(super) fn signature_binding_names(sig: &syn::Signature) -> BTreeSet<String> {
    sig.inputs
        .iter()
        .flat_map(|arg| match arg {
            syn::FnArg::Receiver(_) => Vec::new(),
            syn::FnArg::Typed(arg) => pattern_identifiers(&arg.pat),
        })
        .collect()
}

pub(super) fn pattern_binding_types(
    pattern: &Pat,
    owner: &[String],
    struct_fields: &BTreeMap<(String, String), Option<Vec<String>>>,
) -> BTreeMap<String, PatternBindingType> {
    let mut bindings = BTreeMap::new();
    collect_pattern_binding_types(pattern, owner, struct_fields, &mut bindings);
    bindings
}

fn collect_pattern_binding_types(
    pattern: &Pat,
    owner: &[String],
    struct_fields: &BTreeMap<(String, String), Option<Vec<String>>>,
    bindings: &mut BTreeMap<String, PatternBindingType>,
) {
    match pattern {
        Pat::Ident(value) => {
            bindings.insert(
                value.ident.to_string(),
                PatternBindingType::Local(owner.to_vec()),
            );
            if let Some((_, nested)) = &value.subpat {
                collect_pattern_binding_types(nested, owner, struct_fields, bindings);
            }
        }
        Pat::Struct(value) => {
            for field in &value.fields {
                let syn::Member::Named(field_name) = &field.member else {
                    mark_pattern_unknown(&field.pat, bindings);
                    continue;
                };
                match struct_fields.get(&(owner.join("::"), field_name.to_string())) {
                    Some(Some(field_owner)) => collect_pattern_binding_types(
                        &field.pat,
                        field_owner,
                        struct_fields,
                        bindings,
                    ),
                    Some(None) => mark_pattern_external(&field.pat, bindings),
                    None => mark_pattern_unknown(&field.pat, bindings),
                }
            }
        }
        Pat::Reference(value) => {
            collect_pattern_binding_types(&value.pat, owner, struct_fields, bindings);
        }
        Pat::Type(value) => {
            collect_pattern_binding_types(&value.pat, owner, struct_fields, bindings);
        }
        Pat::Paren(value) => {
            collect_pattern_binding_types(&value.pat, owner, struct_fields, bindings);
        }
        _ => mark_pattern_unknown(pattern, bindings),
    }
}

fn mark_pattern_external(pattern: &Pat, bindings: &mut BTreeMap<String, PatternBindingType>) {
    for name in pattern_identifiers(pattern) {
        bindings.insert(name, PatternBindingType::External);
    }
}

fn mark_pattern_unknown(pattern: &Pat, bindings: &mut BTreeMap<String, PatternBindingType>) {
    for name in pattern_identifiers(pattern) {
        bindings.insert(name, PatternBindingType::Unknown);
    }
}

pub(super) fn expression_starts_with_known_external_receiver(
    expression: &Expr,
    local_types: &BTreeMap<String, Vec<String>>,
    known_external_locals: &BTreeSet<String>,
    repository_types: &BTreeSet<String>,
) -> bool {
    match expression {
        Expr::Path(path) => path.path.get_ident().is_some_and(|identifier| {
            let name = identifier.to_string();
            known_external_locals.contains(&name)
                || local_types
                    .get(&name)
                    .is_some_and(|owner| !repository_types.contains(&owner.join("::")))
        }),
        Expr::MethodCall(call) => expression_starts_with_known_external_receiver(
            &call.receiver,
            local_types,
            known_external_locals,
            repository_types,
        ),
        Expr::Field(field) => expression_starts_with_known_external_receiver(
            &field.base,
            local_types,
            known_external_locals,
            repository_types,
        ),
        Expr::Group(group) => expression_starts_with_known_external_receiver(
            &group.expr,
            local_types,
            known_external_locals,
            repository_types,
        ),
        Expr::Paren(paren) => expression_starts_with_known_external_receiver(
            &paren.expr,
            local_types,
            known_external_locals,
            repository_types,
        ),
        Expr::Reference(reference) => expression_starts_with_known_external_receiver(
            &reference.expr,
            local_types,
            known_external_locals,
            repository_types,
        ),
        Expr::Try(value) => expression_starts_with_known_external_receiver(
            &value.expr,
            local_types,
            known_external_locals,
            repository_types,
        ),
        Expr::Await(value) => expression_starts_with_known_external_receiver(
            &value.base,
            local_types,
            known_external_locals,
            repository_types,
        ),
        _ => false,
    }
}
