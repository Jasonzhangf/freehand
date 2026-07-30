use super::attributes::has_test_attribute;
use super::*;
use crate::openminis_ui_migration::call_graph::cfg::cfg_enabled;
use syn::{ItemImpl, ItemTrait, Stmt, Type};

mod bindings;
mod local_items;
pub(super) mod receiver;
pub(super) mod visitor;
use receiver::{resolve_type_path, type_identity};
pub(super) use visitor::collect_callable;

pub(super) fn collect_initializer(
    ident: &syn::Ident,
    attrs: &[syn::Attribute],
    expression: &syn::Expr,
    module: &[String],
    owner: &str,
    inherited_test: bool,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    visitor::collect_initializer_impl(
        InitializerSource {
            id: format!("{}::{}::__initializer", module.join("::"), ident),
            attrs,
            expression,
            module,
            call_scope: module,
            owner,
            method_owner: None,
            receiver_type: None,
            inherited_test,
        },
        state,
    )
}

pub(super) fn collect_enum_discriminants(
    item: &syn::ItemEnum,
    module: &[String],
    owner: &str,
    inherited_test: bool,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    let enum_is_test = inherited_test || has_test_attribute(&item.attrs, state.active_cfg);
    for variant in &item.variants {
        let Some((_, expression)) = &variant.discriminant else {
            continue;
        };
        visitor::collect_initializer_impl(
            InitializerSource {
                id: format!(
                    "{}::{}::{}::__discriminant_initializer",
                    module.join("::"),
                    item.ident,
                    variant.ident
                ),
                attrs: &variant.attrs,
                expression,
                module,
                call_scope: module,
                owner,
                method_owner: None,
                receiver_type: None,
                inherited_test: enum_is_test,
            },
            state,
        )?;
    }
    Ok(())
}

pub(super) struct InitializerSource<'a> {
    pub(super) id: String,
    pub(super) attrs: &'a [syn::Attribute],
    pub(super) expression: &'a syn::Expr,
    pub(super) module: &'a [String],
    pub(super) call_scope: &'a [String],
    pub(super) owner: &'a str,
    pub(super) method_owner: Option<Vec<String>>,
    pub(super) receiver_type: Option<Vec<String>>,
    pub(super) inherited_test: bool,
}

pub(super) struct CallableSource<'a> {
    pub(super) id: String,
    pub(super) module: &'a [String],
    pub(super) owner: &'a str,
    pub(super) attrs: &'a [syn::Attribute],
    pub(super) sig: &'a syn::Signature,
    pub(super) block: &'a syn::Block,
    pub(super) method_owner: Option<Vec<String>>,
    pub(super) receiver_type: Option<Vec<String>>,
    pub(super) is_test: bool,
}

pub(super) fn inherit_trait_default_dispatch(
    trait_impls: &[(Vec<String>, Vec<String>)],
    trait_default_methods: &BTreeMap<(String, String), String>,
    method_dispatch: &mut BTreeMap<(String, String), BTreeSet<String>>,
) {
    let explicit_dispatch = method_dispatch.keys().cloned().collect::<BTreeSet<_>>();
    for (receiver, trait_path) in trait_impls {
        let receiver = receiver.join("::");
        let trait_path = trait_path.join("::");
        for ((default_trait, method), default_id) in trait_default_methods {
            let dispatch_key = (receiver.clone(), method.clone());
            if default_trait == &trait_path && !explicit_dispatch.contains(&dispatch_key) {
                method_dispatch
                    .entry(dispatch_key)
                    .or_default()
                    .insert(default_id.clone());
            }
        }
    }
}

pub(super) fn record_method_edge(
    function: &RustFunctionCalls,
    name: &str,
    resolved: BTreeSet<String>,
    must_resolve: bool,
    callers: &mut BTreeMap<String, BTreeSet<String>>,
) -> Result<(), String> {
    if resolved.is_empty() {
        if must_resolve {
            return Err(format!(
                "Rust call graph cannot resolve local method call `{name}` from `{}`",
                function.id
            ));
        }
        return Ok(());
    }
    if resolved.len() != 1 {
        return Err(format!(
            "Rust call graph has ambiguous method call `{name}` from `{}`: {resolved:?}",
            function.id
        ));
    }
    let callee = resolved.first().expect("one resolved method");
    callers
        .get_mut(callee)
        .expect("defined method initialized")
        .insert(function.id.clone());
    Ok(())
}

pub(super) fn resolve_method_dispatch(
    receiver: &str,
    method: &str,
    method_dispatch: &BTreeMap<(String, String), BTreeSet<String>>,
    deref_targets: &BTreeMap<String, Vec<String>>,
) -> BTreeSet<String> {
    let mut current = receiver.to_owned();
    let mut visited = BTreeSet::new();
    while visited.insert(current.clone()) {
        if let Some(resolved) = method_dispatch.get(&(current.clone(), method.to_owned()))
            && !resolved.is_empty()
        {
            return resolved.clone();
        }
        let Some(target) = deref_targets.get(&current) else {
            break;
        };
        current = target.join("::");
    }
    BTreeSet::new()
}

fn collect_impl(
    item: &ItemImpl,
    declaration_scope: &[String],
    enclosing_module: &[String],
    owner: &str,
    inherited_test: bool,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    let Type::Path(self_type) = item.self_ty.as_ref() else {
        return Err(format!(
            "unsupported non-path impl type in `{}`",
            declaration_scope.join("::")
        ));
    };
    let type_segments = self_type
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    let receiver_type = resolve_type_path(
        declaration_scope,
        &type_segments,
        state.modules,
        state.local_types,
    );
    let mut method_owner = receiver_type.clone();
    if let Some((_, trait_path, _)) = &item.trait_ {
        let trait_segments = trait_path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
        state.trait_impls.push((
            receiver_type.clone(),
            resolve_type_path(
                declaration_scope,
                &trait_segments,
                state.modules,
                state.local_types,
            ),
        ));
        if trait_segments.last().map(String::as_str) == Some("Deref") {
            let target = item.items.iter().find_map(|item| {
                let syn::ImplItem::Type(associated) = item else {
                    return None;
                };
                (associated.ident == "Target").then_some(&associated.ty)
            });
            if let Some(target) = target.and_then(|target| {
                type_identity(target, declaration_scope, state.modules, state.local_types)
                    .filter(|target| state.local_types.contains(&target.join("::")))
            }) {
                state.deref_targets.insert(receiver_type.join("::"), target);
            }
        }
        let trait_name = trait_path
            .segments
            .last()
            .expect("trait path has segment")
            .ident
            .to_string();
        method_owner.push(format!("impl_{trait_name}"));
    }
    let is_test = inherited_test || has_test_attribute(&item.attrs, state.active_cfg);
    for associated in item.items.iter().filter_map(|item| match item {
        syn::ImplItem::Const(associated) => Some(associated),
        _ => None,
    }) {
        visitor::collect_initializer_impl(
            InitializerSource {
                id: format!(
                    "{}::{}::__initializer",
                    method_owner.join("::"),
                    associated.ident
                ),
                attrs: &associated.attrs,
                expression: &associated.expr,
                module: enclosing_module,
                call_scope: &method_owner,
                owner,
                method_owner: Some(method_owner.clone()),
                receiver_type: Some(receiver_type.clone()),
                inherited_test: is_test,
            },
            state,
        )?;
    }
    for method in item.items.iter().filter_map(|item| match item {
        syn::ImplItem::Fn(method) => Some(method),
        _ => None,
    }) {
        let id = format!("{}::{}", method_owner.join("::"), method.sig.ident);
        if !active_in_cfg_projection(
            &method.attrs,
            &format!("impl method `{id}`"),
            state.active_cfg,
        )? {
            continue;
        }
        state
            .method_dispatch
            .entry((receiver_type.join("::"), method.sig.ident.to_string()))
            .or_default()
            .insert(id.clone());
        let method_is_test = is_test || has_test_attribute(&method.attrs, state.active_cfg);
        collect_local_items(
            &method.block.stmts,
            enclosing_module,
            &id,
            owner,
            method_is_test,
            state,
        )?;
        collect_callable(
            CallableSource {
                id,
                module: enclosing_module,
                owner,
                attrs: &method.attrs,
                sig: &method.sig,
                block: &method.block,
                method_owner: Some(method_owner.clone()),
                receiver_type: Some(receiver_type.clone()),
                is_test: method_is_test,
            },
            state,
        )?;
    }
    Ok(())
}

fn collect_trait(
    item: &ItemTrait,
    declaration_scope: &[String],
    enclosing_module: &[String],
    owner: &str,
    inherited_test: bool,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    let mut trait_owner = declaration_scope.to_vec();
    trait_owner.push(item.ident.to_string());
    state.local_types.insert(trait_owner.join("::"));
    state.local_traits.insert(trait_owner.join("::"));
    let trait_is_test = inherited_test || has_test_attribute(&item.attrs, state.active_cfg);
    for associated in item.items.iter().filter_map(|item| match item {
        syn::TraitItem::Const(associated) => Some(associated),
        _ => None,
    }) {
        if let Some((_, expression)) = &associated.default {
            visitor::collect_initializer_impl(
                InitializerSource {
                    id: format!(
                        "{}::{}::__initializer",
                        trait_owner.join("::"),
                        associated.ident
                    ),
                    attrs: &associated.attrs,
                    expression,
                    module: enclosing_module,
                    call_scope: &trait_owner,
                    owner,
                    method_owner: Some(trait_owner.clone()),
                    receiver_type: Some(trait_owner.clone()),
                    inherited_test: trait_is_test,
                },
                state,
            )?;
        } else {
            active_in_cfg_projection(
                &associated.attrs,
                &format!(
                    "associated const `{}::{}`",
                    trait_owner.join("::"),
                    associated.ident
                ),
                state.active_cfg,
            )?;
        }
    }
    for method in &item.items {
        let syn::TraitItem::Fn(method) = method else {
            continue;
        };
        let id = format!("{}::{}", trait_owner.join("::"), method.sig.ident);
        if !active_in_cfg_projection(
            &method.attrs,
            &format!("trait method `{id}`"),
            state.active_cfg,
        )? {
            continue;
        }
        if let Some(block) = &method.default {
            state.trait_default_methods.insert(
                (trait_owner.join("::"), method.sig.ident.to_string()),
                id.clone(),
            );
            let method_is_test = inherited_test
                || has_test_attribute(&item.attrs, state.active_cfg)
                || has_test_attribute(&method.attrs, state.active_cfg);
            collect_local_items(
                &block.stmts,
                enclosing_module,
                &id,
                owner,
                method_is_test,
                state,
            )?;
            collect_callable(
                CallableSource {
                    id,
                    module: enclosing_module,
                    owner,
                    attrs: &method.attrs,
                    sig: &method.sig,
                    block,
                    method_owner: Some(trait_owner.clone()),
                    receiver_type: Some(trait_owner.clone()),
                    is_test: method_is_test,
                },
                state,
            )?;
        } else if let Some(previous) = state.definitions.insert(id.clone(), owner.to_owned()) {
            return Err(format!(
                "duplicate module-qualified Rust function `{id}` in `{previous}` and `{owner}`"
            ));
        }
    }
    Ok(())
}

pub(super) fn collect_local_items(
    statements: &[Stmt],
    enclosing_module: &[String],
    lexical_owner: &str,
    owner: &str,
    inherited_test: bool,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    local_items::collect_local_items_impl(
        statements,
        enclosing_module,
        lexical_owner,
        owner,
        inherited_test,
        state,
    )
}

pub(super) fn collect_top_level_impl(
    item: &ItemImpl,
    module: &[String],
    owner: &str,
    inherited_test: bool,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    collect_impl(item, module, module, owner, inherited_test, state)
}

pub(super) fn collect_top_level_trait(
    item: &ItemTrait,
    module: &[String],
    owner: &str,
    inherited_test: bool,
    state: &mut DiscoveryState<'_>,
) -> Result<(), String> {
    collect_trait(item, module, module, owner, inherited_test, state)
}
